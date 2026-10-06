package com.loveoverflow.tabula.mobile.design

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * App identity heading from the canonical T Portal mark and outlined wordmark (`assets/brand/`).
 *
 * Geometry and lockup placement are generated from the approved SVGs; color comes only from
 * the generated semantic tokens (doc 04 §8.1). The mark is decorative beside the wordmark:
 * this single canvas exposes one accessible name, without duplicate text or a font fallback.
 * This is shell identity, never a game asset or an occupant avatar.
 */
@Composable
fun TabulaBrand(modifier: Modifier = Modifier, height: Dp = TabulaSpace.xxxxxxl.dp) {
    val colors = LocalTabulaColors.current
    val mark = remember { PathParser().parsePathString(TabulaBrandPaths.Mark).toPath() }
    val wordmark = remember { PathParser().parsePathString(TabulaBrandPaths.Wordmark).toPath() }
    Canvas(
        modifier = modifier
            .size(width = height * (TabulaBrandPaths.LockupWidth / TabulaBrandPaths.LockupHeight), height = height)
            .clearAndSetSemantics {
                contentDescription = "Tabula"
                heading()
            },
    ) {
        val factor = minOf(size.width / TabulaBrandPaths.LockupWidth, size.height / TabulaBrandPaths.LockupHeight)
        scale(factor, factor, pivot = Offset.Zero) {
            drawPath(mark, colors.brandMark)
            translate(TabulaBrandPaths.WordmarkX, TabulaBrandPaths.WordmarkY) {
                drawPath(wordmark, colors.brandWordmark)
            }
        }
    }
}
