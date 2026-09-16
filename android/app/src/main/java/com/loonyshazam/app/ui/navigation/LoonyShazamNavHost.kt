package com.loonyshazam.app.ui.navigation

import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.loonyshazam.app.data.MusicProvider
import com.loonyshazam.app.ui.history.HistoryDetailScreen
import com.loonyshazam.app.ui.history.HistoryScreen
import com.loonyshazam.app.ui.home.HomeScreen
import com.loonyshazam.app.ui.result.ResultScreen
import com.loonyshazam.app.viewmodel.HistoryViewModel
import com.loonyshazam.app.viewmodel.RecognitionViewModel

private object Routes {
    const val HOME = "home"
    const val RESULT = "result"
    const val HISTORY = "history"
    const val HISTORY_DETAIL = "history_detail/{id}"
    fun historyDetail(id: Long) = "history_detail/$id"
}

@Composable
fun LoonyShazamNavHost(
    recognitionViewModel: RecognitionViewModel,
    historyViewModel: HistoryViewModel,
    musicProvider: MusicProvider,
    navController: NavHostController = rememberNavController()
) {
    NavHost(navController = navController, startDestination = Routes.HOME) {
        composable(Routes.HOME) {
            HomeScreen(
                viewModel = recognitionViewModel,
                onNavigateToHistory = { navController.navigate(Routes.HISTORY) },
                onRecognitionSettled = { navController.navigate(Routes.RESULT) }
            )
        }
        composable(Routes.RESULT) {
            val state by recognitionViewModel.uiState.collectAsState()
            ResultScreen(
                state = state,
                musicProvider = musicProvider,
                onSearchAgain = {
                    recognitionViewModel.reset()
                    navController.popBackStack(Routes.HOME, inclusive = false)
                },
                onBack = {
                    recognitionViewModel.reset()
                    navController.popBackStack()
                }
            )
        }
        composable(Routes.HISTORY) {
            HistoryScreen(
                viewModel = historyViewModel,
                onItemClick = { id -> navController.navigate(Routes.historyDetail(id)) },
                onBack = { navController.popBackStack() }
            )
        }
        composable(
            route = Routes.HISTORY_DETAIL,
            arguments = listOf(navArgument("id") { type = NavType.LongType })
        ) { backStackEntry ->
            val id = backStackEntry.arguments?.getLong("id") ?: -1L
            HistoryDetailScreen(
                viewModel = historyViewModel,
                historyId = id,
                musicProvider = musicProvider,
                onBack = { navController.popBackStack() }
            )
        }
    }
}
