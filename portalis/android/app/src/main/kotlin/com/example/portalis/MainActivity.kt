package com.example.portalis

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.embedding.android.FlutterActivity
import io.flutter.plugin.common.MethodChannel
import java.io.ByteArrayOutputStream

class MainActivity: FlutterActivity() {
    private val heicChannel = "app.portalis/heic-preview"

    override fun onCreate(savedInstanceState: android.os.Bundle?) {
        super.onCreate(savedInstanceState)
        PortalisNative.install(applicationContext)
    }

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, heicChannel)
            .setMethodCallHandler { call, result ->
                if (call.method != "decode") {
                    result.notImplemented()
                    return@setMethodCallHandler
                }
                val path = call.argument<String>("path")
                val maxPixelSize = call.argument<Int>("maxPixelSize") ?: 1024
                if (path == null) {
                    result.error("invalid_path", "A HEIC path is required.", null)
                    return@setMethodCallHandler
                }
                Thread {
                    val bytes = decode(path, maxPixelSize)
                    runOnUiThread {
                        if (bytes == null) {
                            result.error(
                                "decode_failed",
                                "The platform could not decode this HEIC image.",
                                null
                            )
                        } else {
                            result.success(bytes)
                        }
                    }
                }.start()
            }
    }

    private fun decode(path: String, maxPixelSize: Int): ByteArray? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(path, bounds)
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null

        val target = maxPixelSize.coerceIn(64, 2048)
        val sample = calculateSample(bounds.outWidth, bounds.outHeight, target)
        val options = BitmapFactory.Options().apply { inSampleSize = sample }
        val bitmap = BitmapFactory.decodeFile(path, options) ?: return null
        return ByteArrayOutputStream().use { output ->
            val compressed = bitmap.compress(Bitmap.CompressFormat.JPEG, 90, output)
            bitmap.recycle()
            if (compressed) output.toByteArray() else null
        }
    }

    private fun calculateSample(width: Int, height: Int, target: Int): Int {
        var sample = 1
        while (width / (sample * 2) >= target && height / (sample * 2) >= target) {
            sample *= 2
        }
        return sample
    }
}

/**
 * Gives the Rust torrent engine an application-scoped Android context once.
 * It does not read media, select files, or copy bytes; those operations stay
 * inside the future URI-backed ContentLocation implementation.
 */
private object PortalisNative {
    init {
        System.loadLibrary("backend")
    }

    @JvmStatic
    fun install(context: Context) {
        installContext(context)
    }

    @JvmStatic
    private external fun installContext(context: Context)
}
