package com.lelloman.debuginterface.internal

import com.lelloman.debuginterface.DebugOperation
import fi.iki.elonen.NanoHTTPD
import fi.iki.elonen.NanoHTTPD.Response

private const val HOME_JS = """
function makeRequest(path) {
    var request = new XMLHttpRequest();
    request.open("POST", path);
    request.onreadystatechange = function() {
        if (request.readyState == XMLHttpRequest.DONE) {
            if (request.status == 200) {
                var data = request.responseText;
                alert(data);
            } else {
                alert("Something went wrong");
            }
        }
    };
    request.send();
}
"""

internal fun home(operations: List<DebugOperation>): Response {
    val commandsList = operations
        .filterIsInstance<DebugOperation.SimpleAction<*>>()
        .joinToString("") { op ->
            "<p><button onclick=\"makeRequest('${op.getKey()}')\">${op.name}</button> ${op.description ?: ""}</p>"
        }
    val page = htmlPage {
        head = "<script>\n$HOME_JS\n</script>"
        body = "<h1>Actions:</h1>$commandsList"
    }
    return NanoHTTPD.newFixedLengthResponse(page)
}