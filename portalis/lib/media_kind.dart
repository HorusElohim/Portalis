import 'package:flutter/material.dart';

/// Shared file-extension classification, used wherever a [MediaItem]'s real
/// downloaded file needs a thumbnail or an appropriate icon.
const imageExtensions = {'jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'heic'};
const videoExtensions = {'mp4', 'mkv', 'mov', 'avi', 'webm', 'm4v'};
const audioExtensions = {'mp3', 'wav', 'flac', 'aac', 'ogg', 'm4a'};
const subtitleExtensions = {'srt', 'vtt', 'ass', 'ssa', 'sub'};

String extensionOf(String name) {
  final dot = name.lastIndexOf('.');
  return dot == -1 ? '' : name.substring(dot + 1).toLowerCase();
}

bool isImage(String name) => imageExtensions.contains(extensionOf(name));
bool isVideo(String name) => videoExtensions.contains(extensionOf(name));
bool isAudio(String name) => audioExtensions.contains(extensionOf(name));
bool isSubtitle(String name) => subtitleExtensions.contains(extensionOf(name));

/// A coarse "what kind of file is this" bucket, driving which icon a
/// [PlaceholderTile] shows when there's no real image thumbnail to render
/// (not downloaded yet, or a type that was never going to have one — a
/// video frame, a subtitle track, an audio file all get their own icon
/// instead of one generic blank tile).
enum MediaKind { image, video, audio, subtitle, other }

MediaKind kindOf(String name) {
  if (isImage(name)) return MediaKind.image;
  if (isVideo(name)) return MediaKind.video;
  if (isAudio(name)) return MediaKind.audio;
  if (isSubtitle(name)) return MediaKind.subtitle;
  return MediaKind.other;
}

IconData iconFor(MediaKind kind) {
  switch (kind) {
    case MediaKind.image:
      return Icons.image_outlined;
    case MediaKind.video:
      return Icons.movie_outlined;
    case MediaKind.audio:
      return Icons.audiotrack_outlined;
    case MediaKind.subtitle:
      return Icons.subtitles_outlined;
    case MediaKind.other:
      return Icons.insert_drive_file_outlined;
  }
}
