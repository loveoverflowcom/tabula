package com.loveoverflow.tabula.mobile.shell

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Matrix
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.catalog.DiscoveryCover
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaShape
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.resources.Res
import com.loveoverflow.tabula.mobile.resources.tabula_discovery_hero
import kotlin.math.tan
import org.jetbrains.compose.resources.painterResource

/** Decorative game-owned cover only; this does not load a runtime asset or any match data (I-5). */
@Composable
fun DiscoveryGameCover(
    cover: DiscoveryCover?,
    modifier: Modifier = Modifier,
    shape: Shape = RoundedCornerShape(topStart = TabulaShape.card.dp, topEnd = TabulaShape.card.dp),
) {
    val colors = LocalTabulaColors.current
    val drawing = remember(cover) {
        cover?.takeIf { it.width.isFinite() && it.height.isFinite() && it.width > 0f && it.height > 0f }
            ?.let { parseDiscoveryArt(it.svg) }
    }
    val ratio = cover?.let { it.width / it.height }?.takeIf { it.isFinite() && it > 0f } ?: (364f / 160f)
    Canvas(modifier.aspectRatio(ratio).clip(shape)
        .clearAndSetSemantics { }) {
        if (cover != null && drawing != null) {
            scale(size.width / cover.width, size.height / cover.height, pivot = Offset.Zero) {
                drawDiscoveryNodes(drawing, colors)
            }
        } else drawNeutralDiscoveryArt(colors)
    }
}

/** Bundled decorative discovery illustration; it grants no catalog or native gameplay availability. */
@Composable
fun DiscoveryHeroArt(modifier: Modifier = Modifier) {
    val colors = LocalTabulaColors.current
    val shape = RoundedCornerShape(TabulaShape.card.dp)
    Image(
        painter = painterResource(Res.drawable.tabula_discovery_hero),
        contentDescription = null,
        // The source is 1376 × 768. Keep its composition at every width and never put UI copy on it.
        contentScale = ContentScale.Crop,
        modifier = modifier.aspectRatio(43f / 24f).clip(shape)
            .border(TabulaSpace.xxs.dp, colors.outline, shape).testTag("discovery-hero-art"),
    )
}

private fun DrawScope.drawNeutralDiscoveryArt(colors: TabulaColors) {
    val factor = minOf(size.width / 364f, size.height / 160f)
    scale(factor, factor, pivot = Offset.Zero) {
        drawRect(colors.shellCoverSage, size = androidx.compose.ui.geometry.Size(364f, 160f))
        val stroke = Stroke(2f)
        withTransform({ rotate(-16f, Offset(167f, 80f)) }) {
            drawRoundRect(colors.primary, Offset(120f, 20f), androidx.compose.ui.geometry.Size(94f, 120f), CornerRadius(12f), style = stroke)
        }
        withTransform({ rotate(14f, Offset(201f, 80f)) }) {
            drawRoundRect(colors.primary, Offset(154f, 20f), androidx.compose.ui.geometry.Size(94f, 120f), CornerRadius(12f), style = stroke)
        }
        drawLine(colors.primary, Offset(181f, 54f), Offset(181f, 106f), 2f)
        drawLine(colors.primary, Offset(155f, 80f), Offset(207f, 80f), 2f)
        drawCircle(colors.primary, 10f, Offset(74f, 58f), style = stroke)
        drawCircle(colors.primary, 14f, Offset(285f, 117f), style = stroke)
        drawLine(colors.primary, Offset(292f, 33f), Offset(292f, 47f), 2f)
        drawLine(colors.primary, Offset(285f, 40f), Offset(299f, 40f), 2f)
    }
}

private data class DiscoveryArtNode(
    val path: Path? = null,
    val fill: String = "none",
    val stroke: String = "none",
    val strokeWidth: Float = 1f,
    val opacity: Float = 1f,
    val matrix: Matrix = Matrix(),
    val children: MutableList<DiscoveryArtNode> = mutableListOf(),
)

/**
 * Small renderer for the registry's static cover vocabulary, not an arbitrary SVG/HTML engine.
 * A malformed, active, external or unsupported element falls back to neutral art. Paths and
 * semantic colors stay game-owned/generated; the mobile shell never chooses shapes by game id.
 */
private fun parseDiscoveryArt(svg: String): List<DiscoveryArtNode>? = runCatching {
    require(svg.length <= 16_384)
    val tokens = Regex("<!--[\\s\\S]*?-->|<[^>]+>").findAll(svg)
    val root = DiscoveryArtNode()
    val stack = mutableListOf(root)
    for (token in tokens) {
        val tag = token.value
        if (tag.startsWith("<!--")) continue
        val closing = tag.startsWith("</")
        val name = Regex("</?([a-zA-Z]+)").find(tag)?.groupValues?.get(1) ?: error("Invalid cover tag")
        if (closing) {
            if (name == "g") { require(stack.size > 1); stack.removeAt(stack.lastIndex) }
            else require(name == "svg")
            continue
        }
        require(name in setOf("svg", "g", "rect", "circle", "path"))
        val attrs = Regex("([a-zA-Z][a-zA-Z0-9:-]*)=\"([^\"]*)\"").findAll(tag)
            .associate { it.groupValues[1] to it.groupValues[2] }
        require(attrs.keys.none { it.startsWith("on", ignoreCase = true) || it in setOf("href", "xlink:href", "style") })
        if (name == "svg") continue
        val parent = stack.last()
        fun number(key: String, default: Float = 0f): Float = attrs[key]?.toFloat()?.also { require(it.isFinite()) } ?: default
        val path = when (name) {
            "path" -> PathParser().parsePathString(attrs["d"] ?: error("Missing path")).toPath()
            "rect" -> Path().apply {
                val x = number("x"); val y = number("y"); val width = number("width"); val height = number("height")
                require(width >= 0f && height >= 0f)
                addRoundRect(RoundRect(x, y, x + width, y + height, CornerRadius(number("rx"), number("ry", number("rx")))))
            }
            "circle" -> Path().apply {
                val x = number("cx"); val y = number("cy"); val radius = number("r")
                require(radius >= 0f)
                addOval(androidx.compose.ui.geometry.Rect(x - radius, y - radius, x + radius, y + radius))
            }
            else -> null
        }
        val node = DiscoveryArtNode(path, attrs["fill"] ?: parent.fill, attrs["stroke"] ?: parent.stroke,
            number("stroke-width", parent.strokeWidth), number("opacity", 1f), parseArtTransform(attrs["transform"] ?: ""))
        parent.children.add(node)
        if (name == "g" && !tag.endsWith("/>")) stack.add(node)
    }
    require(stack.size == 1)
    root.children
}.getOrNull()

private fun parseArtTransform(value: String): Matrix {
    val matrix = Matrix()
    var end = 0
    for (match in Regex("([a-zA-Z]+)\\(([^)]*)\\)").findAll(value)) {
        require(value.substring(end, match.range.first).isBlank())
        end = match.range.last + 1
        val args = match.groupValues[2].trim().split(Regex("[ ,]+"))
            .map { it.toFloat().also { number -> require(number.isFinite()) } }
        when (match.groupValues[1]) {
            "translate" -> { require(args.size in 1..2); matrix.translate(args[0], args.getOrElse(1) { 0f }) }
            "scale" -> { require(args.size in 1..2); matrix.scale(args[0], args.getOrElse(1) { args[0] }) }
            "rotate" -> {
                require(args.size == 1 || args.size == 3)
                if (args.size == 3) matrix.translate(args[1], args[2])
                matrix.rotateZ(args[0])
                if (args.size == 3) matrix.translate(-args[1], -args[2])
            }
            "skewX" -> {
                require(args.size == 1)
                matrix *= Matrix().apply { values[4] = tan(args[0] * (kotlin.math.PI / 180f)).toFloat() }
            }
            else -> error("Unsupported cover transform")
        }
    }
    require(value.substring(end).isBlank())
    return matrix
}

private fun DrawScope.drawDiscoveryNodes(nodes: List<DiscoveryArtNode>, colors: TabulaColors, inheritedOpacity: Float = 1f) {
    for (node in nodes) withTransform({ transform(node.matrix) }) {
        val opacity = (inheritedOpacity * node.opacity).coerceIn(0f, 1f)
        node.path?.let { path ->
            artColor(node.fill, colors)?.let { drawPath(path, it, alpha = opacity) }
            artColor(node.stroke, colors)?.let { drawPath(path, it, alpha = opacity, style = Stroke(node.strokeWidth)) }
        }
        drawDiscoveryNodes(node.children, colors, opacity)
    }
}

private fun artColor(role: String, colors: TabulaColors): Color? = when (role) {
    "none" -> null
    "var(--sys-color-shell-cover-sage)" -> colors.shellCoverSage
    "var(--sys-color-shell-cover-mint)" -> colors.shellCoverMint
    "var(--sys-color-shell-art-field-light)" -> colors.shellArtFieldLight
    "var(--sys-color-shell-art-field-dark)" -> colors.shellArtFieldDark
    "var(--sys-color-shell-art-line)" -> colors.shellArtLine
    "var(--sys-color-shell-art-piece-light)" -> colors.shellArtPieceLight
    "var(--sys-color-shell-art-piece-dark)" -> colors.shellArtPieceDark
    "var(--sys-color-shell-art-road)" -> colors.shellArtRoad
    "var(--sys-color-shell-art-earth)" -> colors.shellArtEarth
    "var(--sys-color-shell-art-piece-red)" -> colors.shellArtPieceRed
    else -> null
}
