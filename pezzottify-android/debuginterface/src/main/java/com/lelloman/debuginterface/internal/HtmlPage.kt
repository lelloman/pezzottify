package com.lelloman.debuginterface.internal

internal class HtmlPage {
    var head: String = ""
    var body: String = ""
}

internal fun htmlPage(builder: HtmlPage.() -> Unit): String {
    val page = HtmlPage()
    builder(page)
    return "<html><head>${page.head}</head><body>${page.body}</body></html>"
}