package app.englishspeaker.data

import android.content.Context
import android.util.Log
import java.io.File

/**
 * 把 assets/models 复制到 filesDir/models（ONNX Runtime 需要真实文件路径）。
 * 用 VERSION_STAMP 判断是否需要重新复制；没有打包模型时返回 null（核心退回 Mock 引擎）。
 */
object ModelInstaller {
    private const val TAG = "ModelInstaller"

    fun install(context: Context): File? {
        val assets = context.assets
        val stamp = runCatching { assets.open("models/VERSION_STAMP").bufferedReader().readText() }.getOrNull()
        val dest = File(context.filesDir, "models")
        if (stamp == null) {
            Log.i(TAG, "no bundled models")
            return dest.takeIf { File(it, "kokoro/model.onnx").isFile }
        }
        val stampFile = File(dest, "VERSION_STAMP")
        if (stampFile.isFile && stampFile.readText() == stamp) return dest

        val t0 = System.currentTimeMillis()
        dest.deleteRecursively()
        copyDir(context, "models", dest)
        stampFile.writeText(stamp)
        Log.i(TAG, "models installed in ${System.currentTimeMillis() - t0}ms")
        return dest
    }

    private fun copyDir(context: Context, path: String, dest: File) {
        val children = context.assets.list(path).orEmpty()
        if (children.isEmpty()) {
            // 文件：先写临时文件再改名，避免中断留下半个模型
            dest.parentFile?.mkdirs()
            val tmp = File(dest.path + ".tmp")
            context.assets.open(path).use { input -> tmp.outputStream().use { input.copyTo(it, 1 shl 16) } }
            tmp.renameTo(dest)
            return
        }
        dest.mkdirs()
        for (c in children) {
            if (c == "VERSION_STAMP") continue
            copyDir(context, "$path/$c", File(dest, c))
        }
    }
}
