#import <Foundation/Foundation.h>
#import <AVFoundation/AVFoundation.h>
#import <Photos/Photos.h>

#include <fcntl.h>
#include <stdio.h>
#include <sys/stat.h>
#include <unistd.h>

static PHAssetResource *PortalisPrimaryResource(PHAsset *asset) {
  NSArray<PHAssetResource *> *resources = [PHAssetResource assetResourcesForAsset:asset];
  NSArray<NSNumber *> *preferred = asset.mediaType == PHAssetMediaTypeVideo
      ? @[@(PHAssetResourceTypeVideo), @(PHAssetResourceTypeFullSizeVideo)]
      : @[@(PHAssetResourceTypeFullSizePhoto), @(PHAssetResourceTypePhoto)];
  for (NSNumber *type in preferred) {
    for (PHAssetResource *resource in resources) {
      if (resource.type == type.integerValue) return resource;
    }
  }
  return nil;
}

static NSURL *PortalisDirectAssetURL(PHAsset *asset, NSError **outError) {
  static NSMutableDictionary<NSString *, NSURL *> *urls;
  static dispatch_once_t once;
  dispatch_once(&once, ^{ urls = [NSMutableDictionary dictionary]; });
  @synchronized (urls) {
    NSURL *cached = urls[asset.localIdentifier];
    if (cached != nil) return cached;
  }

  __block NSURL *url = nil;
  __block NSError *requestError = nil;
  dispatch_semaphore_t done = dispatch_semaphore_create(0);
  if (asset.mediaType == PHAssetMediaTypeVideo) {
    PHVideoRequestOptions *options = [[PHVideoRequestOptions alloc] init];
    options.networkAccessAllowed = YES;
    options.version = PHVideoRequestOptionsVersionOriginal;
    [[PHImageManager defaultManager]
      requestAVAssetForVideo:asset
      options:options
      resultHandler:^(AVAsset *avAsset, AVAudioMix *audioMix, NSDictionary *info) {
        if ([avAsset isKindOfClass:AVURLAsset.class]) {
          url = ((AVURLAsset *)avAsset).URL;
        }
        requestError = info[PHImageErrorKey];
        dispatch_semaphore_signal(done);
      }];
  } else {
    PHContentEditingInputRequestOptions *options = [[PHContentEditingInputRequestOptions alloc] init];
    options.networkAccessAllowed = YES;
    [asset requestContentEditingInputWithOptions:options completionHandler:^(PHContentEditingInput *input, NSDictionary *info) {
      url = input.fullSizeImageURL;
      requestError = info[PHContentEditingInputErrorKey];
      dispatch_semaphore_signal(done);
    }];
  }
  dispatch_semaphore_wait(done, DISPATCH_TIME_FOREVER);
  if (url != nil) {
    @synchronized (urls) { urls[asset.localIdentifier] = url; }
  }
  if (outError != NULL) *outError = requestError;
  return url;
}

static int PortalisReadDirectURL(NSURL *url, uint64_t offset, uint8_t *buffer, size_t length) {
  int descriptor = open(url.fileSystemRepresentation, O_RDONLY);
  if (descriptor < 0) return -1;
  size_t copied = 0;
  while (copied < length) {
    ssize_t count = pread(descriptor, buffer + copied, length - copied, (off_t)(offset + copied));
    if (count <= 0) break;
    copied += (size_t)count;
  }
  close(descriptor);
  return copied == length ? 0 : -1;
}

bool portalis_photo_asset_available(const char *identifier) {
  @autoreleasepool {
    if (identifier == NULL) return false;
    NSString *assetId = [NSString stringWithUTF8String:identifier];
    PHFetchResult<PHAsset *> *assets = [PHAsset fetchAssetsWithLocalIdentifiers:@[assetId] options:nil];
    return assets.count == 1 && PortalisPrimaryResource(assets.firstObject) != nil;
  }
}

int64_t portalis_photo_asset_length(const char *identifier) {
  @autoreleasepool {
    if (identifier == NULL) return -1;
    NSString *assetId = [NSString stringWithUTF8String:identifier];
    PHFetchResult<PHAsset *> *assets = [PHAsset fetchAssetsWithLocalIdentifiers:@[assetId] options:nil];
    PHAsset *asset = assets.count == 1 ? assets.firstObject : nil;
    PHAssetResource *resource = asset == nil ? nil : PortalisPrimaryResource(asset);
    if (resource == nil) return -2;

    NSError *directError = nil;
    NSURL *url = PortalisDirectAssetURL(asset, &directError);
    struct stat attributes;
    if (url != nil && stat(url.fileSystemRepresentation, &attributes) == 0 && attributes.st_size > 0) {
      return attributes.st_size;
    }
    if (directError != nil) {
      fprintf(stderr, "Portalis could not access direct Photos URL for %s: %s\\n", identifier, directError.localizedDescription.UTF8String);
    }

    __block uint64_t received = 0;
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    PHAssetResourceRequestOptions *options = [[PHAssetResourceRequestOptions alloc] init];
    options.networkAccessAllowed = YES;
    [[PHAssetResourceManager defaultManager]
      requestDataForAssetResource:resource
      options:options
      dataReceivedHandler:^(NSData *data) { received += data.length; }
      completionHandler:^(NSError *error) {
        if (error != nil) {
          fprintf(stderr, "Portalis PhotoKit length request for %s completed with error: %s\\n", identifier, error.localizedDescription.UTF8String);
        }
        dispatch_semaphore_signal(done);
      }];
    dispatch_semaphore_wait(done, DISPATCH_TIME_FOREVER);
    return received > 0 && received <= INT64_MAX ? (int64_t)received : -3;
  }
}

// Reads an exact range without materialising the original asset in Portalis'
// container. PhotoKit delivers a sequential stream, so this discards bytes
// before the requested offset and copies only the requested range.
int portalis_photo_asset_read(
    const char *identifier,
    uint64_t offset,
    uint8_t *buffer,
    size_t length) {
  @autoreleasepool {
    if (identifier == NULL || buffer == NULL) return -1;
    NSString *assetId = [NSString stringWithUTF8String:identifier];
    PHFetchResult<PHAsset *> *assets = [PHAsset fetchAssetsWithLocalIdentifiers:@[assetId] options:nil];
    PHAsset *asset = assets.count == 1 ? assets.firstObject : nil;
    PHAssetResource *resource = asset == nil ? nil : PortalisPrimaryResource(asset);
    if (resource == nil) return -2;

    NSError *directError = nil;
    NSURL *url = PortalisDirectAssetURL(asset, &directError);
    if (url != nil && PortalisReadDirectURL(url, offset, buffer, length) == 0) {
      return 0;
    }
    if (directError != nil) {
      fprintf(stderr, "Portalis could not access direct Photos URL for %s: %s\\n", identifier, directError.localizedDescription.UTF8String);
    }

    __block uint64_t position = 0;
    __block size_t copied = 0;
    __block NSError *requestError = nil;
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    PHAssetResourceRequestOptions *options = [[PHAssetResourceRequestOptions alloc] init];
    options.networkAccessAllowed = YES;
    [[PHAssetResourceManager defaultManager]
      requestDataForAssetResource:resource
      options:options
      dataReceivedHandler:^(NSData *data) {
        uint64_t next = position + data.length;
        if (next > offset && copied < length) {
          NSUInteger start = (NSUInteger)MAX((int64_t)0, (int64_t)offset - (int64_t)position);
          NSUInteger count = MIN(data.length - start, length - copied);
          memcpy(buffer + copied, ((const uint8_t *)data.bytes) + start, count);
          copied += count;
        }
        position = next;
      }
      completionHandler:^(NSError *error) {
        requestError = error;
        dispatch_semaphore_signal(done);
      }];
    dispatch_semaphore_wait(done, DISPATCH_TIME_FOREVER);
    if (copied == length) {
      if (requestError != nil) {
        fprintf(stderr, "Portalis received the requested Photos range despite completion error: %s\\n", requestError.localizedDescription.UTF8String);
      }
      return 0;
    }
    if (requestError != nil) {
      fprintf(stderr, "Portalis could not read Photos asset %s at %llu for %zu bytes: %s\\n", identifier, (unsigned long long)offset, length, requestError.localizedDescription.UTF8String);
    } else {
      fprintf(stderr, "Portalis received only %zu of %zu requested bytes from Photos asset %s\\n", copied, length, identifier);
    }
    return -3;
  }
}
