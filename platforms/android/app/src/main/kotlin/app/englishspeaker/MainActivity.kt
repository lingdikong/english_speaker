package app.englishspeaker

import android.Manifest
import android.content.ComponentName
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import app.englishspeaker.core.PlayerStatus
import app.englishspeaker.data.ThemeMode
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import app.englishspeaker.playback.PlaybackService
import app.englishspeaker.ui.AppRoot
import app.englishspeaker.ui.AppViewModel
import app.englishspeaker.ui.theme.SpeakerTheme
import com.google.common.util.concurrent.ListenableFuture
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private val vm: AppViewModel by viewModels()
    private var controller: ListenableFuture<MediaController>? = null
    private val notificationPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { }

    override fun onCreate(savedInstanceState: Bundle?) {
        // 沉浸式：状态栏/导航栏透明，图标颜色在 AppRoot 里跟随主题
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.auto(android.graphics.Color.TRANSPARENT, android.graphics.Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.auto(android.graphics.Color.TRANSPARENT, android.graphics.Color.TRANSPARENT),
        )
        super.onCreate(savedInstanceState)
        if (savedInstanceState == null) vm.applyLaunchArgs(intent.extras)
        setContent {
            val mode by vm.settings.theme.collectAsState()
            val dark = when (mode) {
                ThemeMode.System -> isSystemInDarkTheme()
                ThemeMode.Light -> false
                ThemeMode.Dark -> true
            }
            SpeakerTheme(dark) { AppRoot(vm) }
        }

        // 第一次真正开始播放时再申请通知权限（媒体通知/锁屏控件需要）
        lifecycleScope.launch {
            vm.repo.player.first { it.status == PlayerStatus.PLAYING }
            if (Build.VERSION.SDK_INT >= 33 &&
                ContextCompat.checkSelfPermission(this@MainActivity, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
            ) {
                notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        vm.applyLaunchArgs(intent.extras)
    }

    override fun onStart() {
        super.onStart()
        // 连接 MediaSessionService：服务随之创建，播放时自动转为前台服务并显示媒体通知
        val token = SessionToken(this, ComponentName(this, PlaybackService::class.java))
        controller = MediaController.Builder(this, token).buildAsync()
    }

    override fun onStop() {
        controller?.let { MediaController.releaseFuture(it) }
        controller = null
        super.onStop()
    }
}
