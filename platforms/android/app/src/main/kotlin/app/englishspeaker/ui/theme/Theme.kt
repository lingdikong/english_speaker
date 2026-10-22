package app.englishspeaker.ui.theme

import android.provider.Settings
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.englishspeaker.R

// —— 字体（OFL，随 APK 打包，离线可用）——

/** 标题 / 品牌。 */
val Display = FontFamily(
    Font(R.font.space_grotesk_600, FontWeight.SemiBold),
    Font(R.font.space_grotesk_700, FontWeight.Bold),
)

/** 界面与原文；中文走系统回退。 */
val Ui = FontFamily(
    Font(R.font.manrope_500, FontWeight.Medium),
    Font(R.font.manrope_600, FontWeight.SemiBold),
    Font(R.font.manrope_700, FontWeight.Bold),
)

/** IPA 音标（也用于 ‿ 等 Manrope 没有的符号）。 */
val Ipa = FontFamily(
    Font(R.font.noto_sans_400, FontWeight.Normal),
    Font(R.font.noto_sans_500, FontWeight.Medium),
)

// —— 颜色令牌（UI_SPEC §2.1）——

@Immutable
data class Palette(
    val dark: Boolean,
    val bg: Color,
    val surface: Color,
    val card: Color,
    val control: Color,
    val line: Color,
    val t1: Color,
    val t2: Color,
    val t3: Color,
    val accent: Color,
    val accentInk: Color,
    val accentText: Color,
    val accentSoft: Color,
    val violet: Color,
    val violetText: Color,
    val violetSoft: Color,
    val waveB: Color,
    val waveE: Color,
    val other: Color,
    val pending: Color,
    val dangerText: Color,
    val dangerSoft: Color,
    /** elev 投影色（0 10 30）。 */
    val elevShadow: Color,
    val glass: Color,
    val glassHi: Color,
    val glassEdge: Color,
    /** glass-shadow 投影色（0 14 44）。 */
    val glassShadow: Color,
) {
    /** “非当前句亮度”只改 other 的 alpha。 */
    fun otherAt(alpha: Float): Color = other.copy(alpha = alpha)
}

val DarkPalette = Palette(
    dark = true,
    bg = Color(0xFF11141C), surface = Color(0xFF171B26), card = Color(0xFF1E2330),
    control = Color(0xFF282E3D), line = Color(0xFF262C3A),
    t1 = Color(0xFFD8DCE5), t2 = Color(0xFFA3AABA), t3 = Color(0xFF868E9F),
    accent = Color(0xFF3CCFB9), accentInk = Color(0xFF0E1A18), accentText = Color(0xFF5FD9C6),
    accentSoft = Color(60, 207, 185).copy(alpha = 0.14f),
    violet = Color(0xFFA196F2), violetText = Color(0xFFB7AEF6), violetSoft = Color(161, 150, 242).copy(alpha = 0.14f),
    waveB = Color(0xFF4A5163), waveE = Color(0xFF262B37),
    other = Color(216, 220, 229).copy(alpha = 0.55f), pending = Color(216, 220, 229).copy(alpha = 0.28f),
    dangerText = Color(0xFFF2A7A7), dangerSoft = Color(242, 167, 167).copy(alpha = 0.12f),
    elevShadow = Color.Black.copy(alpha = 0.45f),
    glass = Color(34, 39, 52).copy(alpha = 0.30f), glassHi = Color.White.copy(alpha = 0.10f),
    glassEdge = Color.White.copy(alpha = 0.07f), glassShadow = Color.Black.copy(alpha = 0.50f),
)

val LightPalette = Palette(
    dark = false,
    bg = Color(0xFFF3F4F6), surface = Color(0xFFFAFAFB), card = Color(0xFFFFFFFF),
    control = Color(0xFFE7E9EE), line = Color(0xFFE2E5EA),
    t1 = Color(0xFF1E232E), t2 = Color(0xFF555D6D), t3 = Color(0xFF666E7E),
    accent = Color(0xFF0E9483), accentInk = Color(0xFFFFFFFF), accentText = Color(0xFF08786A),
    accentSoft = Color(14, 148, 131).copy(alpha = 0.12f),
    violet = Color(0xFF6A57D6), violetText = Color(0xFF5A47C8), violetSoft = Color(106, 87, 214).copy(alpha = 0.10f),
    waveB = Color(0xFFBFC4CE), waveE = Color(0xFFE3E6EB),
    other = Color(30, 35, 46).copy(alpha = 0.65f), pending = Color(30, 35, 46).copy(alpha = 0.30f),
    dangerText = Color(0xFFB42828), dangerSoft = Color(180, 40, 40).copy(alpha = 0.08f),
    elevShadow = Color(30, 35, 46).copy(alpha = 0.12f),
    glass = Color.White.copy(alpha = 0.34f), glassHi = Color.White.copy(alpha = 0.95f),
    glassEdge = Color(30, 35, 46).copy(alpha = 0.07f), glassShadow = Color(30, 35, 46).copy(alpha = 0.16f),
)

val LocalPalette = staticCompositionLocalOf { DarkPalette }

/** 是否减少动态效果（系统“移除动画” / 动画时长缩放为 0）。 */
val LocalReduceMotion = staticCompositionLocalOf { false }

// —— 字号（UI_SPEC §2.2）——

object Type {
    /** 大标题 32，Space Grotesk 700，字距 -0.02em。 */
    val largeTitle = TextStyle(fontFamily = Display, fontWeight = FontWeight.Bold, fontSize = 32.sp, lineHeight = 38.sp, letterSpacing = (-0.02).em)
    /** 区块标题 20。 */
    val section = TextStyle(fontFamily = Display, fontWeight = FontWeight.SemiBold, fontSize = 20.sp, lineHeight = 26.sp, letterSpacing = (-0.02).em)
    val sheetTitle = TextStyle(fontFamily = Display, fontWeight = FontWeight.Bold, fontSize = 20.sp, lineHeight = 26.sp, letterSpacing = (-0.02).em)
    /** 卡片标题（继续收听 18 / 播放页 17）。 */
    val displayTitle = TextStyle(fontFamily = Display, fontWeight = FontWeight.Bold, fontSize = 18.sp, lineHeight = 22.sp, letterSpacing = (-0.02).em)
    /** 列表标题 15/700。 */
    val rowTitle = TextStyle(fontFamily = Ui, fontWeight = FontWeight.Bold, fontSize = 15.sp, lineHeight = 20.sp)
    val body = TextStyle(fontFamily = Ui, fontWeight = FontWeight.SemiBold, fontSize = 15.sp, lineHeight = 22.sp)
    val bodyMedium = TextStyle(fontFamily = Ui, fontWeight = FontWeight.Medium, fontSize = 15.sp, lineHeight = 25.sp)
    /** 次要 13。 */
    val secondary = TextStyle(fontFamily = Ui, fontWeight = FontWeight.Medium, fontSize = 13.sp, lineHeight = 18.sp)
    /** 小标签 12/700，字距 0.08em。 */
    val label = TextStyle(fontFamily = Ui, fontWeight = FontWeight.Bold, fontSize = 12.sp, lineHeight = 16.sp, letterSpacing = 0.08.em)
    /** Tab 文字 11/700。 */
    val tab = TextStyle(fontFamily = Ui, fontWeight = FontWeight.Bold, fontSize = 11.sp, lineHeight = 13.sp)
    val chip = TextStyle(fontFamily = Ui, fontWeight = FontWeight.Bold, fontSize = 13.sp, lineHeight = 16.sp)
    val time = TextStyle(fontFamily = Ui, fontWeight = FontWeight.Bold, fontSize = 12.sp, lineHeight = 14.sp)
    val ipa = TextStyle(fontFamily = Ipa, fontWeight = FontWeight.Normal, fontSize = 20.sp, lineHeight = 26.sp)
}

// —— 动效（UI_SPEC §2.3）——

/** 位置/尺寸弹簧：Compose `spring(dampingRatio=0.86f, stiffness=380f)`；减少动态效果时直接切换。 */
fun <T> layoutSpring(reduce: Boolean, visibilityThreshold: T? = null): FiniteAnimationSpec<T> =
    if (reduce) snap() else spring(dampingRatio = 0.86f, stiffness = 380f, visibilityThreshold = visibilityThreshold)

/** 透明度 0.2s。 */
fun <T> fadeSpec(reduce: Boolean, millis: Int = 200): FiniteAnimationSpec<T> = if (reduce) snap() else tween(millis)

@Composable
fun SpeakerTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    val p = if (dark) DarkPalette else LightPalette
    val context = LocalContext.current
    val reduce = remember {
        runCatching {
            Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
        }.getOrDefault(false)
    }
    val scheme = if (dark) {
        darkColorScheme(
            primary = p.accent, onPrimary = p.accentInk, background = p.bg, surface = p.surface,
            onBackground = p.t1, onSurface = p.t1, surfaceContainer = p.surface,
        )
    } else {
        lightColorScheme(
            primary = p.accent, onPrimary = p.accentInk, background = p.bg, surface = p.surface,
            onBackground = p.t1, onSurface = p.t1, surfaceContainer = p.surface,
        )
    }
    CompositionLocalProvider(LocalPalette provides p, LocalReduceMotion provides reduce) {
        MaterialTheme(colorScheme = scheme, content = content)
    }
}
