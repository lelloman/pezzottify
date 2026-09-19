package com.lelloman.pezzottify.android.ui.component

import com.lelloman.androidoscopy.session.PairingRequest
import com.lelloman.androidoscopy.session.SessionState
import org.junit.Assert.assertEquals
import org.junit.Test

class DiagnosticSessionStatusTest {
    @Test fun `inactive includes stopped and expired`() {
        assertEquals(DiagnosticSessionStatus.OFF, diagnosticSessionStatus(SessionState(reason = "Session expired")))
    }
    @Test fun `active session without a peer is waiting`() {
        assertEquals(DiagnosticSessionStatus.WAITING, diagnosticSessionStatus(SessionState(active = true)))
    }
    @Test fun `pending pairing overrides a prior failure`() {
        assertEquals(DiagnosticSessionStatus.PAIRING, diagnosticSessionStatus(SessionState(active = true,
            pairing = PairingRequest("id", "code", "address"), reason = "old error")))
    }
    @Test fun `connected peer overrides a stale error`() {
        assertEquals(DiagnosticSessionStatus.CONNECTED, diagnosticSessionStatus(SessionState(active = true,
            peer = "pc", reason = "old error")))
    }
    @Test fun `connection failure is distinct from waiting`() {
        assertEquals(DiagnosticSessionStatus.ATTENTION, diagnosticSessionStatus(SessionState(active = true,
            reason = "PC disconnected. Waiting for a connection.")))
    }
}
