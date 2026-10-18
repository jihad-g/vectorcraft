package ai.storyteller.vectorcraft

import android.annotation.SuppressLint
import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Base64
import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.MimeTypeMap
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Toast
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.webkit.WebViewAssetLoader
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import java.io.File
import java.io.InputStream

/**
 * VectorCraft for Android: the browser build (dist/web, bundled under assets/web) in a WebView.
 *
 * - The page is served from https://appassets.androidplatform.net/ by WebViewAssetLoader, a secure
 *   origin, so WebAssembly streaming, fetch() of the bundled fonts, and the clipboard work.
 * - Open / Place: the page's file input opens the system file picker (onShowFileChooser).
 * - Save / Export: the web build saves by "downloading" a blob: URL. A small script hooks that
 *   download, hands the bytes to [Bridge.save], and the system "Save as" picker writes them.
 * - Links to other sites open in the phone's browser.
 * - Runs offline: no INTERNET permission.
 */
class MainActivity : AppCompatActivity() {

    private lateinit var web: WebView
    private var fileCallback: ValueCallback<Array<Uri>>? = null

    /** A save waiting for the user to pick where it goes. */
    private var pendingSave: Pair<String, ByteArray>? = null
    private var lastBack = 0L

    private val openFiles = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        val cb = fileCallback ?: return@registerForActivityResult
        fileCallback = null
        cb.onReceiveValue(WebChromeClient.FileChooserParams.parseResult(result.resultCode, result.data))
    }

    private val createDocument = registerForActivityResult(ActivityResultContracts.CreateDocument("*/*")) { uri ->
        val (name, bytes) = pendingSave ?: return@registerForActivityResult
        pendingSave = null
        if (uri == null) return@registerForActivityResult
        val ok = runCatching {
            contentResolver.openOutputStream(uri, "wt")?.use { it.write(bytes) } ?: error("no stream")
        }.isSuccess
        toast(getString(if (ok) R.string.saved else R.string.save_failed, name))
    }

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        WebView.setWebContentsDebuggingEnabled(BuildConfig.DEBUG)
        web = WebView(this)
        web.setBackgroundColor(0xFF262626.toInt())
        web.overScrollMode = View.OVER_SCROLL_NEVER
        setContentView(web)

        web.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true // Data Recovery and preferences use localStorage / IndexedDB
            databaseEnabled = true
            allowFileAccess = false
            allowContentAccess = false
            mediaPlaybackRequiresUserGesture = true
            setSupportZoom(false) // the app does its own pinch zoom
            builtInZoomControls = false
            displayZoomControls = false
            useWideViewPort = true
            loadWithOverviewMode = false
            cacheMode = WebSettings.LOAD_NO_CACHE // everything is local
            textZoom = 100 // the system font size must not rescale the canvas UI
        }

        val loader = WebViewAssetLoader.Builder().addPathHandler("/assets/", TypedAssetsHandler()).build()

        web.addJavascriptInterface(Bridge(), "VectorCraftAndroid")
        if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            WebViewCompat.addDocumentStartJavaScript(web, SAVE_HOOK, setOf(ORIGIN))
        }

        web.webViewClient = object : WebViewClient() {
            override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? =
                loader.shouldInterceptRequest(request.url)

            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                val url = request.url
                if (url.host == HOST) return false
                // Help pages, Discord, etc.: the phone's browser.
                try {
                    startActivity(Intent(Intent.ACTION_VIEW, url))
                } catch (_: ActivityNotFoundException) {
                }
                return true
            }

            override fun onPageFinished(view: WebView, url: String) {
                // Older WebViews without document-start scripts: hook saves once the page is up
                // (the app takes a moment to start, so this lands before the first save).
                if (!WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
                    view.evaluateJavascript(SAVE_HOOK, null)
                }
            }
        }

        web.webChromeClient = object : WebChromeClient() {
            override fun onShowFileChooser(view: WebView, callback: ValueCallback<Array<Uri>>, params: FileChooserParams): Boolean {
                fileCallback?.onReceiveValue(null)
                fileCallback = callback
                val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    // The page filters by extension (.svg, .pdf, .vectorcraft…), which Android's
                    // MIME types can't express for every format: show everything.
                    type = "*/*"
                    putExtra(Intent.EXTRA_ALLOW_MULTIPLE, params.mode == FileChooserParams.MODE_OPEN_MULTIPLE)
                }
                return try {
                    openFiles.launch(intent)
                    true
                } catch (_: ActivityNotFoundException) {
                    fileCallback = null
                    false
                }
            }
        }

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                // Back = Escape first (closes menus, the panel sheet, dialogs); twice quickly exits.
                web.evaluateJavascript(ESCAPE, null)
                val now = System.currentTimeMillis()
                if (now - lastBack < 1500) {
                    finish()
                } else {
                    lastBack = now
                    toast(getString(R.string.back_again))
                }
            }
        })

        if (savedInstanceState != null) {
            web.restoreState(savedInstanceState)
        } else {
            web.loadUrl("$ORIGIN/assets/web/index.html")
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        web.saveState(outState)
    }

    override fun onDestroy() {
        web.destroy()
        super.onDestroy()
    }

    private fun toast(text: String) = Toast.makeText(this, text, Toast.LENGTH_SHORT).show()

    /** Called from the page's save hook (on a WebView thread). */
    inner class Bridge {
        @JavascriptInterface
        fun save(name: String, mime: String, base64: String) {
            val bytes = try {
                Base64.decode(base64, Base64.DEFAULT)
            } catch (_: IllegalArgumentException) {
                runOnUiThread { toast(getString(R.string.save_failed, name)) }
                return
            }
            val clean = File(name).name.ifBlank { "untitled" }
            runOnUiThread {
                pendingSave = clean to bytes
                try {
                    createDocument.launch(clean)
                } catch (_: ActivityNotFoundException) {
                    pendingSave = null
                    toast(getString(R.string.save_failed, clean))
                }
            }
        }
    }

    /** Bundled assets with the MIME types the page needs (.wasm must be application/wasm). */
    private inner class TypedAssetsHandler : WebViewAssetLoader.PathHandler {
        override fun handle(path: String): WebResourceResponse? {
            val stream: InputStream = try {
                assets.open(path)
            } catch (_: Exception) {
                return WebResourceResponse("text/plain", "utf-8", 404, "Not Found", emptyMap(), null)
            }
            val ext = path.substringAfterLast('.', "").lowercase()
            val mime = when (ext) {
                "wasm" -> "application/wasm"
                "js", "mjs" -> "text/javascript"
                "html" -> "text/html"
                "css" -> "text/css"
                "json", "webmanifest" -> "application/json"
                "svg" -> "image/svg+xml"
                "ttf" -> "font/ttf"
                "otf" -> "font/otf"
                "woff2" -> "font/woff2"
                else -> MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext) ?: "application/octet-stream"
            }
            val text = mime.startsWith("text/") || mime == "application/json"
            return WebResourceResponse(mime, if (text) "utf-8" else null, stream)
        }
    }

    companion object {
        private const val HOST = "appassets.androidplatform.net"
        private const val ORIGIN = "https://$HOST"

        /**
         * The web build saves with `<a download href="blob:…">.click()`. Read that blob and pass
         * it to the app instead (WebView can't download blob: URLs by itself). fetch() resolves
         * the blob URL synchronously, before the page revokes it.
         */
        private val SAVE_HOOK = """
            (function () {
              if (window.__vcSaveHook) return;
              window.__vcSaveHook = true;
              var click = HTMLAnchorElement.prototype.click;
              HTMLAnchorElement.prototype.click = function () {
                var href = this.href || '';
                if (this.hasAttribute('download') && href.indexOf('blob:') === 0 && window.VectorCraftAndroid) {
                  var name = this.download || 'untitled';
                  fetch(href).then(function (r) { return r.blob(); }).then(function (b) {
                    var fr = new FileReader();
                    fr.onload = function () {
                      var s = String(fr.result), i = s.indexOf(',');
                      window.VectorCraftAndroid.save(name, b.type || '', i >= 0 ? s.slice(i + 1) : '');
                    };
                    fr.readAsDataURL(b);
                  });
                  return;
                }
                return click.call(this);
              };
            })();
        """.trimIndent()

        private const val ESCAPE =
            "(function(){var c=document.getElementById('vectorcraft_canvas')||document.body;" +
                "['keydown','keyup'].forEach(function(t){c.dispatchEvent(new KeyboardEvent(t,{key:'Escape',code:'Escape',bubbles:true}));});})();"
    }
}
