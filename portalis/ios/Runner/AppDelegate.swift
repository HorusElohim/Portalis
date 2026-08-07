import Flutter
import Network
import UIKit

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  private let noCopySourcePicker = NoCopySourcePicker()
  private let photoLibraryPicker = PhotoLibraryPicker()
  private let localNetworkAuthorization = LocalNetworkAuthorization()

  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)
    guard let registrar = engineBridge.pluginRegistry.registrar(
      forPlugin: "PortalisNoCopySourcePicker"
    ) else {
      return
    }
    noCopySourcePicker.register(with: registrar)
    photoLibraryPicker.register(with: registrar)
    localNetworkAuthorization.request()
  }
}

private final class LocalNetworkAuthorization {
  private var browser: NWBrowser?

  func request() {
    guard browser == nil else { return }
    let browser = NWBrowser(
      for: .bonjour(type: "_portalis._tcp", domain: nil),
      using: .tcp
    )
    browser.stateUpdateHandler = { state in
      if case let .failed(error) = state {
        NSLog("Portalis Local Network browser failed: %@", error.localizedDescription)
      }
    }
    self.browser = browser
    browser.start(queue: .main)
  }
}
