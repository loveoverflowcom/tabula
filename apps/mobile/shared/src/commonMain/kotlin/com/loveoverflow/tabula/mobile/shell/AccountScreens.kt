package com.loveoverflow.tabula.mobile.shell

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.account.AccountErrorReason
import com.loveoverflow.tabula.mobile.account.AccountIdentity
import com.loveoverflow.tabula.mobile.account.AccountOperation
import com.loveoverflow.tabula.mobile.account.AccountProfileVisibility
import com.loveoverflow.tabula.mobile.account.AccountState
import com.loveoverflow.tabula.mobile.account.AccountUnavailableReason
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.design.TabulaText
import com.loveoverflow.tabula.mobile.design.TabulaType
import com.loveoverflow.tabula.mobile.localization.AccountCopy
import com.loveoverflow.tabula.mobile.localization.ShellCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.localization.account
import com.loveoverflow.tabula.mobile.navigation.Destination
import com.loveoverflow.tabula.mobile.navigation.AccountTaskBackPort

/** Public action ports: screens render supplied facts and never synthesize identity or authority. */
class AccountActions(
    val navigate: (Destination) -> Unit,
    val refresh: () -> Unit,
    val signOut: () -> Unit,
    val cancel: () -> Unit,
    val back: AccountTaskBackPort = AccountTaskBackPort(),
)

/** Compact mobile account entry with current, typed session state (ADR-0046). */
@Composable
fun AccountScreen(state: AccountState, strings: ShellStrings, actions: AccountActions, avatar: AccountAvatarImage? = null) {
    AccountPage(strings[ShellCopy.AccountTitle], "shell-account") {
        TabulaText(strings.account(AccountCopy.Intro), TabulaType.bodyLg, color = LocalTabulaColors.current.onSurfaceVariant)
        AccountIdentityCard(state, strings, avatar)
        AccountStatusPanel(state, strings, actions, accountEntry = true)
        if (state !is AccountState.Authenticated) AccountAccessLinks(strings, actions)
        AccountDestinationCard(AccountCopy.Profile, AccountCopy.ProfileActionBody, AccountCopy.ProfileAction,
            Destination.Profile, "account-open-profile", strings, actions)
        AccountDestinationCard(AccountCopy.Friends, AccountCopy.FriendsActionBody, AccountCopy.Friends,
            Destination.Friends, "account-open-friends", strings, actions)
        if (state is AccountState.Authenticated) SignOutControls(state, strings, actions)
        AccountLocalEscape(strings, actions)
        TabulaText(strings.account(AccountCopy.Preferences), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
    }
}

/** Sign-in is an explicit capability surface until a verified native provider adapter exists. */
@Composable
fun LoginScreen(state: AccountState, strings: ShellStrings, actions: AccountActions) {
    AccountPage(strings.account(AccountCopy.Login), "shell-login") {
        TabulaText(strings.account(AccountCopy.LoginIntro), TabulaType.bodyLg, color = LocalTabulaColors.current.onSurfaceVariant)
        AccountStatusPanel(state, strings, actions)
        if (state is AccountState.Authenticated) {
            ShellSurface {
                TabulaText(strings.account(AccountCopy.AlreadySignedIn), TabulaType.bodyMd)
                AccountAction(AccountCopy.ProfileAction, "account-open-profile", strings,
                    { actions.navigate(Destination.Profile) }, ShellAction.Filled)
            }
        } else {
            ShellStatePanel(strings.account(AccountCopy.LoginUnavailable), strings.account(AccountCopy.LoginUnavailableBody),
                Modifier.testTag("account-login-unavailable"))
            TabulaText(strings.account(AccountCopy.ProviderBoundary), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
            AccountAction(AccountCopy.Register, "account-open-register", strings, { actions.navigate(Destination.Register) })
        }
        AccountLocalEscape(strings, actions)
    }
}

/** Enrollment has its own truthful route; no local form, password or accepted-account simulation. */
@Composable
fun RegisterScreen(state: AccountState, strings: ShellStrings, actions: AccountActions) {
    AccountPage(strings.account(AccountCopy.Register), "shell-register") {
        TabulaText(strings.account(AccountCopy.RegisterIntro), TabulaType.bodyLg, color = LocalTabulaColors.current.onSurfaceVariant)
        if (state is AccountState.Authenticated) {
            ShellSurface {
                TabulaText(strings.account(AccountCopy.AlreadySignedIn), TabulaType.bodyMd)
                AccountAction(AccountCopy.ProfileAction, "account-open-profile", strings,
                    { actions.navigate(Destination.Profile) }, ShellAction.Filled)
            }
        }
        ShellStatePanel(strings.account(AccountCopy.RegisterUnavailable), strings.account(AccountCopy.RegisterUnavailableBody),
            Modifier.testTag("account-register-unavailable"))
        TabulaText(strings.account(AccountCopy.RegistrationDisposition), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
        if (state !is AccountState.Authenticated) {
            AccountAction(AccountCopy.Login, "account-open-login", strings, { actions.navigate(Destination.Login) })
        }
        AccountLocalEscape(strings, actions)
    }
}

/** Permitted read-only self fields only; the route never resolves a different profile or grants edit. */
@Composable
fun ProfileScreen(state: AccountState, strings: ShellStrings, actions: AccountActions, avatar: AccountAvatarImage? = null) {
    AccountPage(strings.account(AccountCopy.Profile), "shell-profile") {
        TabulaText(strings.account(AccountCopy.ProfileIntro), TabulaType.bodyLg, color = LocalTabulaColors.current.onSurfaceVariant)
        AccountStatusPanel(state, strings, actions)
        if (state is AccountState.Authenticated) {
            AccountIdentityCard(state, strings, avatar)
            ShellSurface(Modifier.testTag("profile-read-only")) {
                TabulaText(strings.account(AccountCopy.ReadOnly), TabulaType.titleMd, Modifier.semantics { heading() })
                val profile = state.identity.profile
                if (profile != null) {
                    ProfileFact(AccountCopy.DisplayName, profile.displayName, "profile-display-name", strings)
                    ProfileFact(AccountCopy.Handle, "@${profile.handle}", "profile-handle", strings)
                    ProfileFact(AccountCopy.Visibility, strings.account(when (profile.visibility) {
                        AccountProfileVisibility.Public -> AccountCopy.VisibilityPublic
                        AccountProfileVisibility.Friends -> AccountCopy.VisibilityFriends
                        AccountProfileVisibility.Private -> AccountCopy.VisibilityPrivate
                    }), "profile-visibility", strings)
                }
                // The opaque immutable identifier is labelled separately, never used as a name.
                ProfileFact(AccountCopy.AccountId, state.identity.accountId.value, "profile-account-id", strings)
            }
            if (state.identity.profile == null) {
                ShellStatePanel(strings.account(AccountCopy.ProfileMissing), strings.account(AccountCopy.ProfileMissingBody),
                    Modifier.testTag("profile-details-unavailable"))
            }
            SignOutControls(state, strings, actions)
        } else AccountAccessLinks(strings, actions)
        TabulaText(strings.account(AccountCopy.OtherProfilesUnavailable), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
        AccountLocalEscape(strings, actions)
    }
}

/** No directory/requests/presence is fabricated merely because a session is authenticated. */
@Composable
fun FriendsScreen(state: AccountState, strings: ShellStrings, actions: AccountActions) {
    AccountPage(strings.account(AccountCopy.Friends), "shell-friends") {
        TabulaText(strings.account(AccountCopy.FriendsIntro), TabulaType.bodyLg, color = LocalTabulaColors.current.onSurfaceVariant)
        ShellStatePanel(strings.account(AccountCopy.FriendsUnavailable), strings.account(AccountCopy.FriendsUnavailableBody),
            Modifier.testTag("account-friends-unavailable"))
        if (state !is AccountState.Authenticated) AccountAccessLinks(strings, actions)
        else AccountAction(AccountCopy.ProfileAction, "account-open-profile", strings, { actions.navigate(Destination.Profile) })
        AccountLocalEscape(strings, actions)
    }
}

/** Safe status also used by the account toolbar; no identity value or raw transport error is copy. */
fun accountStatusCopy(state: AccountState): AccountCopy = when (state) {
    AccountState.Unknown -> AccountCopy.Checking
    is AccountState.Loading -> if (state.operation == AccountOperation.SignOut) AccountCopy.SigningOut else AccountCopy.Loading
    AccountState.SignedOut -> AccountCopy.SignedOut
    is AccountState.Authenticated -> AccountCopy.SignedIn
    AccountState.Expired -> AccountCopy.Expired
    is AccountState.Unavailable -> AccountCopy.Unavailable
    is AccountState.Error -> AccountCopy.Error
}

@Composable
private fun AccountPage(title: String, tag: String, content: @Composable androidx.compose.foundation.layout.ColumnScope.() -> Unit) {
    ShellPage(title, Modifier.testTag(tag).semantics { paneTitle = title }, content = content)
}

@Composable
private fun AccountIdentityCard(state: AccountState, strings: ShellStrings, avatar: AccountAvatarImage?) {
    val identity = (state as? AccountState.Authenticated)?.identity
    ShellSurface(Modifier.testTag("account-identity"), hero = true) {
        Row(horizontalArrangement = Arrangement.spacedBy(TabulaSpace.lg.dp), verticalAlignment = Alignment.CenterVertically) {
            ShellIdentityAvatar(identity, avatar, strings)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(TabulaSpace.xs.dp)) {
                TabulaText(identity?.profile?.displayName ?: if (identity == null) strings[ShellCopy.Anonymous]
                    else strings.account(AccountCopy.SignedIn), TabulaType.titleLg)
                TabulaText(identity?.profile?.let { "@${it.handle}" } ?: strings.account(AccountCopy.Status),
                    TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
            }
        }
    }
}

@Composable
private fun AccountStatusPanel(state: AccountState, strings: ShellStrings, actions: AccountActions, accountEntry: Boolean = false) {
    val tag = when (state) {
        AccountState.Unknown -> "unknown"
        is AccountState.Loading -> "loading"
        AccountState.SignedOut -> "signed-out"
        is AccountState.Authenticated -> "authenticated"
        AccountState.Expired -> "expired"
        is AccountState.Unavailable -> "unavailable"
        is AccountState.Error -> "error"
    }
    val message = when (state) {
        AccountState.Unknown -> AccountCopy.Checking
        is AccountState.Loading -> if (state.operation == AccountOperation.SignOut) AccountCopy.SigningOut else AccountCopy.Loading
        AccountState.SignedOut -> AccountCopy.SignedOutBody
        is AccountState.Authenticated -> AccountCopy.SignedInBody
        AccountState.Expired -> AccountCopy.ExpiredBody
        is AccountState.Unavailable -> when (state.reason) {
            AccountUnavailableReason.NativeAdapterMissing -> AccountCopy.NativeUnavailableBody
            AccountUnavailableReason.ServiceUnavailable -> AccountCopy.BackendUnavailableBody
            AccountUnavailableReason.SecureStorageUnavailable -> AccountCopy.StorageUnavailableBody
        }
        is AccountState.Error -> if (state.reason == AccountErrorReason.SignOutUnconfirmed) AccountCopy.SignOutUnconfirmedBody else AccountCopy.ErrorBody
    }
    val loading = state is AccountState.Loading
    ShellSurface(Modifier.testTag("account-state-$tag").semantics { liveRegion = LiveRegionMode.Polite }) {
        TabulaText(strings.account(accountStatusCopy(state)), TabulaType.titleMd,
            Modifier.semantics { heading(); if (loading) progressBarRangeInfo = ProgressBarRangeInfo.Indeterminate },
            if (state is AccountState.Error || state == AccountState.Expired) LocalTabulaColors.current.danger else LocalTabulaColors.current.onSurface)
        if (!loading && state != AccountState.Unknown) {
            TabulaText(strings.account(message), TabulaType.bodyMd,
                if (accountEntry && state is AccountState.Unavailable) Modifier.testTag("shell-account-unavailable") else Modifier,
                LocalTabulaColors.current.onSurfaceVariant)
        }
        when (state) {
            is AccountState.Loading -> AccountAction(if (state.operation == AccountOperation.Refresh)
                AccountCopy.Cancel else AccountCopy.StopWaiting, "account-cancel", strings, actions.cancel, ShellAction.Text)
            is AccountState.Authenticated -> AccountAction(AccountCopy.Refresh, "account-retry", strings, actions.refresh)
            is AccountState.Error -> when (state.retry) {
                AccountOperation.Refresh -> AccountAction(AccountCopy.CheckSession, "account-retry", strings, actions.refresh, ShellAction.Filled)
                AccountOperation.SignOut -> AccountAction(AccountCopy.RetrySignOut, "account-retry", strings, actions.signOut, ShellAction.Filled)
                null -> Unit
            }
            is AccountState.Unavailable -> if (state.reason != AccountUnavailableReason.NativeAdapterMissing) {
                AccountAction(AccountCopy.CheckSession, "account-retry", strings, actions.refresh)
            }
            else -> AccountAction(AccountCopy.CheckSession, "account-retry", strings, actions.refresh, ShellAction.Filled)
        }
    }
}

@Composable
private fun ProfileFact(label: AccountCopy, value: String, tag: String, strings: ShellStrings) {
    Column(Modifier.fillMaxWidth().testTag(tag), verticalArrangement = Arrangement.spacedBy(TabulaSpace.xs.dp)) {
        TabulaText(strings.account(label), TabulaType.labelLg, color = LocalTabulaColors.current.onSurfaceVariant)
        TabulaText(value, TabulaType.bodyLg)
    }
}

@Composable
private fun AccountDestinationCard(title: AccountCopy, body: AccountCopy, action: AccountCopy, destination: Destination,
    tag: String, strings: ShellStrings, actions: AccountActions) {
    ShellSurface {
        TabulaText(strings.account(title), TabulaType.titleMd, Modifier.semantics { heading() })
        TabulaText(strings.account(body), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
        AccountAction(action, tag, strings, { actions.navigate(destination) })
    }
}

@Composable
private fun AccountAccessLinks(strings: ShellStrings, actions: AccountActions) {
    Column(verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp)) {
        AccountAction(AccountCopy.Login, "account-open-login", strings, { actions.navigate(Destination.Login) })
        AccountAction(AccountCopy.Register, "account-open-register", strings, { actions.navigate(Destination.Register) }, ShellAction.Text)
    }
}

@Composable
private fun AccountLocalEscape(strings: ShellStrings, actions: AccountActions) {
    ShellSurface {
        TabulaText(strings.account(AccountCopy.LocalTitle), TabulaType.titleMd, Modifier.semantics { heading() })
        TabulaText(strings.account(AccountCopy.LocalBody), TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
        AccountAction(AccountCopy.Library, "account-browse-library", strings, { actions.navigate(Destination.Games) }, ShellAction.Filled)
    }
}

@Composable
private fun AccountAction(copy: AccountCopy, tag: String, strings: ShellStrings, onClick: () -> Unit,
    style: ShellAction = ShellAction.Tonal, modifier: Modifier = Modifier) {
    ShellActionButton(strings.account(copy), style, onClick, modifier.fillMaxWidth().testTag(tag))
}

/**
 * Local confirmation exists only while this exact identity remains current; it is never saved.
 * Native Back/Escape cancels it first. Pending sign-out is masked by the session owner, not undone.
 */
@OptIn(ExperimentalComposeUiApi::class)
@Suppress("DEPRECATION")
@Composable
private fun SignOutControls(state: AccountState.Authenticated, strings: ShellStrings, actions: AccountActions) {
    if (!state.canSignOut) return
    val identityKey = IdentityPresentationKey(state.identity)
    var confirming by remember(identityKey) { mutableStateOf(false) }
    var returnFocus by remember(identityKey) { mutableStateOf(false) }
    val invoker = remember { FocusRequester() }
    val confirm = remember { FocusRequester() }
    val cancel = { confirming = false; returnFocus = true }
    BackHandler(enabled = confirming, onBack = cancel)
    DisposableEffect(actions.back, identityKey, confirming) {
        val release = if (confirming) actions.back.register { cancel(); true } else null
        onDispose { release?.invoke() }
    }
    LaunchedEffect(confirming, returnFocus) {
        if (confirming) confirm.requestFocus()
        else if (returnFocus) { invoker.requestFocus(); returnFocus = false }
    }
    if (!confirming) {
        AccountAction(AccountCopy.SignOut, "account-sign-out", strings, { confirming = true }, ShellAction.Text,
            Modifier.focusRequester(invoker))
    } else {
        ShellSurface(Modifier.testTag("account-sign-out-confirmation").onPreviewKeyEvent { event ->
            if (event.key == Key.Escape && event.type == KeyEventType.KeyDown) { cancel(); true } else false
        }) {
            TabulaText(strings.account(AccountCopy.ConfirmSignOut), TabulaType.titleMd, Modifier.semantics { heading() })
            TabulaText(strings.account(AccountCopy.ConfirmSignOutBody), TabulaType.bodyMd)
            AccountAction(AccountCopy.ConfirmSignOutAction, "account-confirm-sign-out", strings,
                { confirming = false; actions.signOut() }, ShellAction.Filled, Modifier.focusRequester(confirm))
            AccountAction(AccountCopy.KeepSignedIn, "account-cancel-sign-out", strings, cancel)
        }
    }
}

/** Compose remember compares keys by equality; presentation bindings require reference identity. */
private class IdentityPresentationKey(private val identity: AccountIdentity) {
    override fun equals(other: Any?): Boolean = other is IdentityPresentationKey && other.identity === identity
    override fun hashCode(): Int = identity.hashCode()
}
