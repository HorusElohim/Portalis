package com.example.portalis

import android.content.Context
import io.flutter.embedding.android.FlutterActivity

class MainActivity: FlutterActivity() {
    override fun onCreate(savedInstanceState: android.os.Bundle?) {
        super.onCreate(savedInstanceState)
        PortalisNative.install(applicationContext)
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
