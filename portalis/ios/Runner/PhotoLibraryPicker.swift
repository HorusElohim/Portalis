import AVKit
import Flutter
import Photos
import PhotosUI
import UIKit

/// Selects Photos-library assets by stable PhotoKit identifier, never by cache path.
final class PhotoLibraryPicker: NSObject, PHPickerViewControllerDelegate {
  private static let channelName = "app.portalis/photo-library-picker"
  private var registrar: FlutterPluginRegistrar?
  private weak var presenter: UIViewController?
  private var pendingResult: FlutterResult?

  func register(with registrar: FlutterPluginRegistrar) {
    self.registrar = registrar
    presenter = registrar.viewController
    let channel = FlutterMethodChannel(name: Self.channelName, binaryMessenger: registrar.messenger())
    channel.setMethodCallHandler { [weak self] call, result in
      guard let self else {
        result(FlutterError(code: "picker_unavailable", message: "Portalis could not open the Photos picker.", details: nil))
        return
      }
      DispatchQueue.main.async {
        switch call.method {
        case "pickMedia": self.open(result: result)
        case "previewMedia": self.preview(call.arguments, result: result)
        default: result(FlutterMethodNotImplemented)
        }
      }
    }
  }

  private func open(result: @escaping FlutterResult) {
    guard #available(iOS 14, *) else {
      result(FlutterError(code: "ios_version_unsupported", message: "Selecting Photos assets requires iOS 14 or later.", details: nil))
      return
    }
    guard pendingResult == nil else {
      result(FlutterError(code: "picker_unavailable", message: "Portalis could not open the Photos picker.", details: nil))
      return
    }
    let status = PHPhotoLibrary.authorizationStatus(for: .readWrite)
    if status == .notDetermined {
      PHPhotoLibrary.requestAuthorization(for: .readWrite) { [weak self] status in
        DispatchQueue.main.async { self?.openAfterAuthorization(status, result: result) }
      }
      return
    }
    openAfterAuthorization(status, result: result)
  }

  @available(iOS 14, *)
  private func openAfterAuthorization(_ status: PHAuthorizationStatus, result: @escaping FlutterResult) {
    guard status == .authorized || status == .limited else {
      result(FlutterError(code: "photos_access_denied", message: "Allow Photos access to seed selected media in place.", details: nil))
      return
    }
    guard let presenter = visiblePresenter() else {
      result(FlutterError(code: "picker_unavailable", message: "Portalis could not open the Photos picker.", details: nil))
      return
    }
    pendingResult = result
    var configuration = PHPickerConfiguration(photoLibrary: .shared())
    configuration.selectionLimit = 0
    configuration.filter = .any(of: [.images, .videos])
    let picker = PHPickerViewController(configuration: configuration)
    picker.delegate = self
    presenter.present(picker, animated: true)
  }

  @available(iOS 14, *)
  func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
    picker.dismiss(animated: true)
    let identifiers = results.compactMap(\.assetIdentifier)
    guard identifiers.count == results.count else {
      complete(FlutterError(code: "asset_identifier_unavailable", message: "Portalis needs Photos access to retain selected media.", details: nil))
      return
    }
    guard !identifiers.isEmpty else { complete([]); return }
    Task.detached { [weak self] in
      let assets = PHAsset.fetchAssets(withLocalIdentifiers: identifiers, options: nil)
      var items: [[String: Any]] = []
      assets.enumerateObjects { asset, _, _ in
        let resource = PHAssetResource.assetResources(for: asset).first { resource in
          asset.mediaType == .video
            ? resource.type == .video || resource.type == .fullSizeVideo
            : resource.type == .fullSizePhoto || resource.type == .photo
        }
        guard let resource else { return }
        items.append([
          "name": resource.originalFilename,
          "path": "phasset://\(asset.localIdentifier)",
          "lengthBytes": Self.resourceLength(resource),
        ])
      }
      DispatchQueue.main.async {
        guard items.count == identifiers.count else {
          self?.complete(FlutterError(code: "asset_unavailable", message: "A selected Photos item is no longer available.", details: nil))
          return
        }
        self?.complete(items)
      }
    }
  }

  @available(iOS 14, *)
  private static func resourceLength(_ resource: PHAssetResource) -> Int64 {
    let semaphore = DispatchSemaphore(value: 0)
    let options = PHAssetResourceRequestOptions()
    options.isNetworkAccessAllowed = true
    var length: Int64 = 0
    PHAssetResourceManager.default().requestData(
      for: resource,
      options: options,
      dataReceivedHandler: { data in length += Int64(data.count) },
      completionHandler: { _ in semaphore.signal() }
    )
    semaphore.wait()
    return length
  }

  private func preview(_ arguments: Any?, result: @escaping FlutterResult) {
    guard #available(iOS 14, *) else {
      result(FlutterError(code: "ios_version_unsupported", message: "Previewing Photos assets requires iOS 14 or later.", details: nil))
      return
    }
    guard let values = arguments as? [String: Any],
          let sourcePath = values["path"] as? String,
          sourcePath.hasPrefix("phasset://"),
          let presenter = visiblePresenter() else {
      result(FlutterError(code: "preview_unavailable", message: "Portalis could not open this Photos item.", details: nil))
      return
    }
    let identifier = String(sourcePath.dropFirst("phasset://".count))
    let assets = PHAsset.fetchAssets(withLocalIdentifiers: [identifier], options: nil)
    guard let asset = assets.firstObject else {
      result(FlutterError(code: "asset_unavailable", message: "This Photos item is no longer available.", details: nil))
      return
    }
    if asset.mediaType == .video {
      previewVideo(asset, presenter: presenter, result: result)
    } else {
      previewImage(asset, presenter: presenter, result: result)
    }
  }

  private func previewImage(_ asset: PHAsset, presenter: UIViewController, result: @escaping FlutterResult) {
    let options = PHImageRequestOptions()
    options.isNetworkAccessAllowed = true
    options.deliveryMode = .highQualityFormat
    options.resizeMode = .none
    PHImageManager.default().requestImage(
      for: asset,
      targetSize: PHImageManagerMaximumSize,
      contentMode: .aspectFit,
      options: options
    ) { [weak self] image, info in
      DispatchQueue.main.async {
        guard let image else {
          let error = info?[PHImageErrorKey] as? Error
          result(FlutterError(code: "preview_failed", message: error?.localizedDescription ?? "Photos could not render this image.", details: nil))
          return
        }
        let controller = UIViewController()
        controller.view.backgroundColor = .black
        let imageView = UIImageView(image: image)
        imageView.contentMode = .scaleAspectFit
        imageView.translatesAutoresizingMaskIntoConstraints = false
        controller.view.addSubview(imageView)
        NSLayoutConstraint.activate([
          imageView.leadingAnchor.constraint(equalTo: controller.view.leadingAnchor),
          imageView.trailingAnchor.constraint(equalTo: controller.view.trailingAnchor),
          imageView.topAnchor.constraint(equalTo: controller.view.topAnchor),
          imageView.bottomAnchor.constraint(equalTo: controller.view.bottomAnchor),
        ])
        let close = UIButton(type: .system)
        close.setImage(UIImage(systemName: "xmark.circle.fill"), for: .normal)
        close.tintColor = .white
        close.translatesAutoresizingMaskIntoConstraints = false
        guard let self else {
          result(FlutterError(code: "preview_unavailable", message: "Portalis could not show this image.", details: nil))
          return
        }
        close.addTarget(self, action: #selector(self.dismissPreview), for: .touchUpInside)
        controller.view.addSubview(close)
        NSLayoutConstraint.activate([
          close.trailingAnchor.constraint(equalTo: controller.view.safeAreaLayoutGuide.trailingAnchor, constant: -16),
          close.topAnchor.constraint(equalTo: controller.view.safeAreaLayoutGuide.topAnchor, constant: 12),
          close.widthAnchor.constraint(equalToConstant: 38),
          close.heightAnchor.constraint(equalToConstant: 38),
        ])
        controller.modalPresentationStyle = .fullScreen
        presenter.present(controller, animated: true)
        result(nil)
      }
    }
  }

  private func previewVideo(_ asset: PHAsset, presenter: UIViewController, result: @escaping FlutterResult) {
    let options = PHVideoRequestOptions()
    options.isNetworkAccessAllowed = true
    options.version = .original
    PHImageManager.default().requestPlayerItem(forVideo: asset, options: options) { item, info in
      DispatchQueue.main.async {
        guard let item else {
          let error = info?[PHImageErrorKey] as? Error
          result(FlutterError(code: "preview_failed", message: error?.localizedDescription ?? "Photos could not load this video.", details: nil))
          return
        }
        let controller = AVPlayerViewController()
        controller.player = AVPlayer(playerItem: item)
        presenter.present(controller, animated: true) { controller.player?.play() }
        result(nil)
      }
    }
  }

  @objc private func dismissPreview() {
    visiblePresenter()?.dismiss(animated: true)
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
