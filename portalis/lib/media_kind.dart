/// Shared file-extension classification, used wherever a [MediaItem]'s real
/// downloaded file needs a thumbnail or an appropriate icon.
const imageExtensions = {'jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'heic'};
const videoExtensions = {'mp4', 'mkv', 'mov', 'avi', 'webm', 'm4v'};
const audioExtensions = {'mp3', 'wav', 'flac', 'aac', 'ogg', 'm4a'};

String extensionOf(String name) {
  final dot = name.lastIndexOf('.');
  return dot == -1 ? '' : name.substring(dot + 1).toLowerCase();
}

bool isImage(String name) => imageExtensions.contains(extensionOf(name));
bool isVideo(String name) => videoExtensions.contains(extensionOf(name));
bool isAudio(String name) => audioExtensions.contains(extensionOf(name));
