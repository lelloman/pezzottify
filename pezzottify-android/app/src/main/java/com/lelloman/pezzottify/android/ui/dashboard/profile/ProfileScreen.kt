package com.lelloman.pezzottify.android.ui.dashboard.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.constraintlayout.compose.ConstrainScope
import androidx.constraintlayout.compose.ConstraintLayout
import androidx.hilt.navigation.compose.hiltViewModel

fun ConstrainScope.linkParentHorizontal() {
    start.linkTo(parent.start)
    end.linkTo(parent.end)
}

@Composable
fun ProfileScreen(
    viewModel: ProfileViewModel = hiltViewModel(),
) {
    ConstraintLayout(modifier = Modifier.fillMaxSize()) {
        val (label, button) = createRefs()

        val labelConstraint = Modifier.constrainAs(label) {
            top.linkTo(parent.top)
            linkParentHorizontal()
        }
        Text("Profile this", modifier = labelConstraint.background(color = Color.Cyan).padding(96.dp))

        val buttonConstraint = Modifier.constrainAs(button) {
            top.linkTo(label.bottom)
            linkParentHorizontal()
            bottom.linkTo(parent.bottom)
        }
        Button({ viewModel.onLogoutClicked() }, modifier = buttonConstraint) {
            Text("Logout")
        }
    }
}