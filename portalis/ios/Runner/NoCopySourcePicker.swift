import Flutter
import UIKit

/// Bridges the iOS Files document picker without allowing it to import a copy.
///
/// URLs selected with the picker are security-scoped. Their bookmarks are kept
/// in UserDefaults and resolved when the Flutter engine starts, so Rust can
/// keep reading the original Files-provider location through app restarts.
final class NoCopySourcePicker: NSObject, UIDocumentPickerDelegate {
  private static let channelName = "app.portalis/no-copy-source-picker"
  private static let bookmarksKey = "portalis.no-copy-source-bookmarks"

  private var registrar: FlutterPluginRegistrar?
  private weak var presenter: UIViewController?
  private var channel: FlutterMethodChannel?
  private var pendingResult: FlutterResult?
  private var activeURLs: [String: URL] = [:]

  func register(with registrar: FlutterPluginRegistrar) {
    self.registrar = registrar
    presenter = registrar.viewController
    restoreAccess()

    let channel = FlutterMethodChannel(
      name: Self.channelName,
      binaryMessenger: registrar.messenger()
    )
    channel.setMethodCallHandler { [weak self] call, result in
      self?.handle(call, result: result)
    }
    self.channel = channel
  }

  private func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard call.method == "pickFiles" else {
      result(FlutterMethodNotImplemented)
      return
    }
    guard pendingResult == nil else {
      result(FlutterError(
        code: "picker_busy",
        message: "A Files selection is already open.",
        details: nil
      ))
      return
    }
    DispatchQueue.main.async { [weak self] in
      self?.presentPicker(result: result)
    }
  }

  private func presentPicker(result: @escaping FlutterResult) {
    guard let presenter = visiblePresenter() else {
      result(FlutterError(
        code: "picker_unavailable",
        message: "Portalis could not open the Files picker.",
        details: nil
      ))
      return
    }

    pendingResult = result
    let picker = UIDocumentPickerViewController(
      documentTypes: ["public.item"],
      in: .open
    )
    picker.allowsMultipleSelection = true
    picker.delegate = self
    presenter.present(picker, animated: true)
  }

  func documentPicker(
    _ controller: UIDocumentPickerViewController,
    didPickDocumentsAt urls: [URL]
  ) {
    do {
      complete(try urls.map(adopt))
    } catch {
      complete(FlutterError(
        code: "selection_failed",
        message: error.localizedDescription,
        details: nil
      ))
    }
  }

  func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
    complete(nil)
  }

  private func adopt(_ url: URL) throws -> [String: Any] {
    let path = url.path
    let alreadyActive = activeURLs[path] != nil
    let startedAccess = !alreadyActive && url.startAccessingSecurityScopedResource()
    do {
      guard FileManager.default.fileExists(atPath: path) else {
        throw PickerError.unavailable(path)
      }
      var isDirectory = ObjCBool(false)
      FileManager.default.fileExists(atPath: path, isDirectory: &isDirectory)
      guard !isDirectory.boolValue else {
        throw PickerError.directory(path)
      }
      guard startedAccess || alreadyActive || isInPortalisDocuments(url) else {
        throw PickerError.accessDenied(path)
      }

      if startedAccess {
        activeURLs[path] = url
        try saveBookmark(for: url, path: path)
      }
      let attributes = try FileManager.default.attributesOfItem(atPath: path)
      let length = (attributes[.size] as? NSNumber)?.int64Value ?? 0
      return [
        "name": url.lastPathComponent,
        "path": path,
        "lengthBytes": length,
      ]
    } catch {
      if startedAccess {
        activeURLs.removeValue(forKey: path)
        url.stopAccessingSecurityScopedResource()
      }
      throw error
    }
  }

  private func isInPortalisDocuments(_ url: URL) -> Bool {
    guard let documents = FileManager.default.urls(
      for: .documentDirectory,
      in: .userDomainMask
    ).first else {
      return false
    }
    return url.path == documents.path || url.path.hasPrefix(documents.path + "/")
  }

  private func saveBookmark(for url: URL, path: String) throws {
    let data = try url.bookmarkData(
      options: [],
      includingResourceValuesForKeys: nil,
      relativeTo: nil
    )
    var bookmarks = savedBookmarks
    bookmarks[path] = data
    UserDefaults.standard.set(bookmarks, forKey: Self.bookmarksKey)
  }

  private var savedBookmarks: [String: Data] {
    guard let values = UserDefaults.standard.dictionary(forKey: Self.bookmarksKey) else {
      return [:]
    }
    return values.reduce(into: [:]) { bookmarks, entry in
      if let data = entry.value as? Data {
        bookmarks[entry.key] = data
      }
    }
  }

  private func restoreAccess() {
    for (path, bookmark) in savedBookmarks {
      var stale = false
      guard let url = try? URL(
        resolvingBookmarkData: bookmark,
        options: [],
        relativeTo: nil,
        bookmarkDataIsStale: &stale
      ), url.startAccessingSecurityScopedResource() else {
        continue
      }
      activeURLs[path] = url
      if stale {
        try? saveBookmark(for: url, path: path)
      }
    }
  }

  private func visiblePresenter() -> UIViewController? {
    if let presenter = registrar?.viewController, presenter.viewIfLoaded?.window != nil {
      return topmostPresenter(from: presenter)
    }
    if let presenter, presenter.viewIfLoaded?.window != nil {
      return topmostPresenter(from: presenter)
    }
    let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.sorted {
      $0.activationState == .foregroundActive && $1.activationState != .foregroundActive
    }
    for scene in scenes {
      let window = scene.windows.first(where: \.isKeyWindow) ?? scene.windows.first(where: { !$0.isHidden })
      if let root = window?.rootViewController {
        return topmostPresenter(from: root)
      }
    }
    return UIApplication.shared.windows.first(where: \.isKeyWindow)?.rootViewController.map(topmostPresenter)
  }

  private func topmostPresenter(from controller: UIViewController) -> UIViewController {
    if let presented = controller.presentedViewController {
      return topmostPresenter(from: presented)
    }
    if let navigation = controller as? UINavigationController,
       let visible = navigation.visibleViewController {
      return topmostPresenter(from: visible)
    }
    if let tabs = controller as? UITabBarController,
       let selected = tabs.selectedViewController {
      return topmostPresenter(from: selected)
    }
    return controller
  }

  private func complete(_ value: Any?) {
    let result = pendingResult
    pendingResult = nil
    result?(value)
  }
}

private enum PickerError: LocalizedError {
  case unavailable(String)
  case directory(String)
  case accessDenied(String)

  var errorDescription: String? {
    switch self {
    case let .unavailable(path):
      return "The selected file is no longer available: \(path)"
    case let .directory(path):
      return "Choose files, not a folder: \(path)"
    case let .accessDenied(path):
      return "Portalis needs Files access to seed this item: \(path)"
    }
  }
}
