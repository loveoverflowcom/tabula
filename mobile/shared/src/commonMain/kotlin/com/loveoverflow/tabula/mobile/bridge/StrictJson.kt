package com.loveoverflow.tabula.mobile.bridge

/** A parsed JSON value. Only what the bridge grammar needs: integers, no floats. */
internal sealed interface Json {
    data class Obj(val fields: Map<String, Json>) : Json
    data class Arr(val items: List<Json>) : Json
    data class Str(val value: String) : Json
    data class Num(val value: Long) : Json
    data class Bool(val value: Boolean) : Json
    data object Null : Json
}

/**
 * A deliberately narrow JSON reader for untrusted bridge text (ADR-0033).
 *
 * It exists because a general parser accepts things the bridge must refuse: duplicate keys
 * (a parser differential between the page's author and this host), floats and exponents,
 * leading zeros, raw control characters, ill-formed surrogates and unbounded nesting.
 * Any violation returns `null`; this never throws on hostile input.
 */
internal object StrictJson {
    const val MAX_DEPTH = 4
    private const val MAX_DIGITS = 15 // every integer stays exactly representable in a JS number

    /** [maxDepth] defaults to the wire grammar's bound; only trusted fixtures may raise it. */
    fun parse(text: String, maxDepth: Int = MAX_DEPTH): Json? = try {
        Reader(text, maxDepth).document()
    } catch (_: Reject) {
        null
    }

    private class Reject : Exception()

    private class Reader(private val text: String, private val maxDepth: Int) {
        private var at = 0

        fun document(): Json {
            val value = value(0)
            whitespace()
            if (at != text.length) reject()
            return value
        }

        private fun reject(): Nothing = throw Reject()

        private fun whitespace() {
            while (at < text.length && text[at].let { it == ' ' || it == '\t' || it == '\n' || it == '\r' }) at++
        }

        private fun peek(): Char = if (at < text.length) text[at] else reject()

        private fun expect(char: Char) {
            if (peek() != char) reject()
            at++
        }

        private fun literal(word: String) {
            if (!text.startsWith(word, at)) reject()
            at += word.length
        }

        private fun value(depth: Int): Json {
            if (depth > maxDepth) reject()
            whitespace()
            return when (peek()) {
                '{' -> obj(depth)
                '[' -> arr(depth)
                '"' -> Json.Str(string())
                't' -> { literal("true"); Json.Bool(true) }
                'f' -> { literal("false"); Json.Bool(false) }
                'n' -> { literal("null"); Json.Null }
                else -> number()
            }
        }

        private fun obj(depth: Int): Json {
            expect('{')
            val fields = LinkedHashMap<String, Json>()
            whitespace()
            if (peek() == '}') { at++; return Json.Obj(fields) }
            while (true) {
                whitespace()
                val key = string()
                whitespace()
                expect(':')
                if (fields.containsKey(key)) reject()
                fields[key] = value(depth + 1)
                whitespace()
                when (peek()) {
                    ',' -> at++
                    '}' -> { at++; return Json.Obj(fields) }
                    else -> reject()
                }
            }
        }

        private fun arr(depth: Int): Json {
            expect('[')
            val items = ArrayList<Json>()
            whitespace()
            if (peek() == ']') { at++; return Json.Arr(items) }
            while (true) {
                items += value(depth + 1)
                whitespace()
                when (peek()) {
                    ',' -> at++
                    ']' -> { at++; return Json.Arr(items) }
                    else -> reject()
                }
            }
        }

        private fun number(): Json {
            val start = at
            if (peek() == '-') at++
            val digitsStart = at
            when (peek()) {
                '0' -> at++
                in '1'..'9' -> while (at < text.length && text[at] in '0'..'9') at++
                else -> reject()
            }
            if (at - digitsStart > MAX_DIGITS) reject()
            // Fractions, exponents and a digit run after a leading zero are not integers.
            if (at < text.length && (text[at] == '.' || text[at] == 'e' || text[at] == 'E' || text[at] in '0'..'9')) reject()
            return Json.Num(text.substring(start, at).toLong())
        }

        private fun string(): String {
            expect('"')
            val out = StringBuilder()
            while (true) {
                val char = peek()
                at++
                when {
                    char == '"' -> return out.toString()
                    char < ' ' -> reject()
                    char == '\\' -> out.append(escape())
                    char.isHighSurrogate() -> {
                        if (at >= text.length || !text[at].isLowSurrogate()) reject()
                        out.append(char).append(text[at]); at++
                    }
                    char.isLowSurrogate() -> reject()
                    else -> out.append(char)
                }
            }
        }

        private fun escape(): Char = when (peek().also { at++ }) {
            '"' -> '"'
            '\\' -> '\\'
            '/' -> '/'
            'b' -> '\b'
            'f' -> '\u000C'
            'n' -> '\n'
            'r' -> '\r'
            't' -> '\t'
            // Surrogate escapes are refused outright: the page writes non-BMP text as raw UTF-8
            // and replaces lone surrogates before sending, so neither form can be legitimate.
            'u' -> unicode().also { unit -> if (unit.isSurrogate()) reject() }
            else -> reject()
        }

        private fun unicode(): Char {
            if (at + 4 > text.length) reject()
            var value = 0
            repeat(4) {
                val digit = text[at].digitToIntOrNull(16) ?: reject()
                value = value * 16 + digit
                at++
            }
            return value.toChar()
        }
    }
}
