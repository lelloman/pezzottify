<template>
  <div class="userManagement">
    <header class="pageHeader">
      <h2 class="sectionTitle">Users</h2>
      <p>Manage accounts, roles and access.</p>
    </header>

    <!-- Create User Section -->
    <div class="createUserSection">
      <input
        v-model="newUserHandle"
        type="text"
        placeholder="New user handle"
        aria-label="New user handle"
        class="createUserInput"
        @keyup.enter="handleCreateUser"
      />
      <button
        class="createUserButton"
        @click="handleCreateUser"
        :disabled="!newUserHandle.trim() || isCreating"
      >
        {{ isCreating ? "Creating..." : "Create user" }}
      </button>
    </div>
    <div v-if="createError" class="createError">{{ createError }}</div>

    <div v-if="isLoading" class="loadingState">Loading users...</div>

    <div v-else-if="loadError" class="errorState">
      {{ loadError }}
      <button class="retryButton" @click="loadUsers">Retry</button>
    </div>

    <div v-else class="userList">
      <div v-for="user in users" :key="user.user_handle" class="userCard">
        <div class="userHeader">
          <button
            class="userToggle"
            :aria-expanded="Boolean(expandedUsers[user.user_handle])"
            @click="toggleUserExpanded(user.user_handle)"
          >
            <span class="userAvatar" aria-hidden="true">{{
              user.user_handle.slice(0, 1).toUpperCase()
            }}</span>
            <span class="userName">{{ user.user_handle }}</span>
            <svg
              class="expandIcon"
              :class="{ expanded: expandedUsers[user.user_handle] }"
              viewBox="0 0 24 24"
              aria-hidden="true"
            >
              <path d="m8 5 7 7-7 7" />
            </svg>
          </button>
          <button
            class="deleteUserButton"
            @click="initiateDelete(user.user_handle)"
            :aria-label="`Delete ${user.user_handle}`"
            title="Delete user"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 10v7M14 10v7" />
            </svg>
          </button>
        </div>

        <div v-if="expandedUsers[user.user_handle]" class="userDetails">
          <!-- Loading state for user details -->
          <div
            v-if="loadingUserDetails[user.user_handle]"
            class="detailsLoading"
          >
            Loading details...
          </div>

          <template v-else>
            <!-- Roles Section -->
            <div class="detailSection">
              <h4 class="detailTitle">Roles</h4>
              <div class="roleList">
                <span
                  v-for="role in userDetails[user.user_handle]?.roles || []"
                  :key="role"
                  class="roleTag"
                >
                  {{ role }}
                  <button
                    class="removeButton"
                    @click="handleRemoveRole(user.user_handle, role)"
                    :aria-label="`Remove ${role} role from ${user.user_handle}`"
                    title="Remove role"
                  >
                    ×
                  </button>
                </span>
                <span
                  v-if="!userDetails[user.user_handle]?.roles?.length"
                  class="emptyState"
                  >No roles</span
                >
              </div>
              <div class="addRoleForm">
                <select
                  v-model="newRole[user.user_handle]"
                  class="roleSelect"
                  :aria-label="`Add role for ${user.user_handle}`"
                >
                  <option value="">Add role...</option>
                  <option value="Admin">Admin</option>
                  <option value="Regular">Regular</option>
                </select>
                <button
                  v-if="newRole[user.user_handle]"
                  class="addButton"
                  @click="handleAddRole(user.user_handle)"
                >
                  Add
                </button>
              </div>
            </div>

            <!-- Permissions Section -->
            <div class="detailSection">
              <h4 class="detailTitle">Current permissions</h4>
              <div class="permissionList">
                <span
                  v-for="perm in userDetails[user.user_handle]?.permissions ||
                  []"
                  :key="perm"
                  class="permissionTag"
                >
                  {{ perm }}
                </span>
                <span
                  v-if="!userDetails[user.user_handle]?.permissions?.length"
                  class="emptyState"
                  >No permissions</span
                >
              </div>
            </div>

            <!-- Grant Permission Section -->
            <div class="detailSection">
              <h4 class="detailTitle">Grant extra permission</h4>
              <div class="grantForm">
                <select
                  v-model="grantPermission[user.user_handle]"
                  class="permissionSelect"
                  aria-label="Permission to grant"
                >
                  <option value="">Select permission...</option>
                  <option
                    v-for="perm in availablePermissions"
                    :key="perm"
                    :value="perm"
                  >
                    {{ perm }}
                  </option>
                </select>
                <input
                  v-model.number="grantDuration[user.user_handle]"
                  type="number"
                  min="0"
                  placeholder="Duration (seconds)"
                  aria-label="Permission duration in seconds"
                  class="durationInput"
                />
                <input
                  v-model.number="grantCountdown[user.user_handle]"
                  type="number"
                  min="0"
                  placeholder="Countdown (uses)"
                  aria-label="Permission use count"
                  class="countdownInput"
                />
                <button
                  v-if="grantPermission[user.user_handle]"
                  class="grantButton"
                  @click="handleGrantPermission(user.user_handle)"
                >
                  Grant
                </button>
              </div>
              <p class="grantHint">
                Leave duration and countdown empty for permanent permission.
              </p>
            </div>

            <!-- Password login Section -->
            <div class="detailSection">
              <h4 class="detailTitle">Password login</h4>
              <div class="passwordStatus">
                <span
                  v-if="userDetails[user.user_handle]?.hasPassword"
                  class="statusBadge hasPassword"
                >
                  Password set
                </span>
                <span v-else class="statusBadge noPassword"> No password </span>
              </div>
              <div class="passwordForm">
                <input
                  v-model="newPassword[user.user_handle]"
                  type="password"
                  :placeholder="
                    userDetails[user.user_handle]?.hasPassword
                      ? 'New password...'
                      : 'Set password...'
                  "
                  class="passwordInput"
                  aria-label="New password"
                  autocomplete="new-password"
                  @keyup.enter="handleSetPassword(user.user_handle)"
                />
                <button
                  v-if="newPassword[user.user_handle]"
                  class="setPasswordButton"
                  @click="handleSetPassword(user.user_handle)"
                  :disabled="settingPassword[user.user_handle]"
                >
                  {{
                    settingPassword[user.user_handle]
                      ? "Saving..."
                      : userDetails[user.user_handle]?.hasPassword
                        ? "Update"
                        : "Set"
                  }}
                </button>
                <button
                  v-if="
                    userDetails[user.user_handle]?.hasPassword &&
                    !newPassword[user.user_handle]
                  "
                  class="removePasswordButton"
                  @click="handleRemovePassword(user.user_handle)"
                  :disabled="settingPassword[user.user_handle]"
                >
                  Remove
                </button>
              </div>
              <div v-if="passwordError[user.user_handle]" class="passwordError">
                {{ passwordError[user.user_handle] }}
              </div>
            </div>
          </template>
        </div>
      </div>

      <div v-if="users.length === 0" class="emptyUsers">No users found.</div>
    </div>

    <!-- First Confirmation Dialog -->
    <div
      v-if="showFirstConfirm"
      class="dialogOverlay"
      @click.self="cancelDelete"
    >
      <div
        class="dialogBox"
        role="dialog"
        aria-modal="true"
        aria-labelledby="delete-dialog-title"
        @keydown.esc="cancelDelete"
      >
        <h3 id="delete-dialog-title" class="dialogTitle">Delete user</h3>
        <p class="dialogMessage">
          Are you sure you want to delete user
          <strong>{{ deleteTarget }}</strong
          >?
        </p>
        <div class="dialogActions">
          <button class="dialogButton cancelButton" @click="cancelDelete">
            Cancel
          </button>
          <button class="dialogButton dangerButton" @click="confirmFirstDelete">
            Delete
          </button>
        </div>
      </div>
    </div>

    <!-- Second Confirmation Dialog -->
    <div
      v-if="showSecondConfirm"
      class="dialogOverlay"
      @click.self="cancelDelete"
    >
      <div
        class="dialogBox"
        role="dialog"
        aria-modal="true"
        aria-labelledby="delete-dialog-title"
        @keydown.esc="cancelDelete"
      >
        <h3 id="delete-dialog-title" class="dialogTitle">Confirm deletion</h3>
        <p class="dialogMessage">
          This will permanently delete <strong>{{ deleteTarget }}</strong> and
          all their data (playlists, liked content, settings, etc.).
        </p>
        <p class="dialogMessage">
          Type <strong>{{ deleteTarget }}</strong> to confirm:
        </p>
        <input
          v-model="confirmDeleteName"
          type="text"
          class="confirmInput"
          aria-label="Confirm user handle"
          :placeholder="deleteTarget"
          @keyup.enter="confirmSecondDelete"
        />
        <div class="dialogActions">
          <button class="dialogButton cancelButton" @click="cancelDelete">
            Cancel
          </button>
          <button
            class="dialogButton dangerButton"
            @click="confirmSecondDelete"
            :disabled="confirmDeleteName !== deleteTarget"
          >
            Delete permanently
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted } from "vue";
import { useRemoteStore } from "@/store/remote";

const remoteStore = useRemoteStore();

const users = ref([]);
const isLoading = ref(true);
const loadError = ref(null);

// Create user state
const newUserHandle = ref("");
const isCreating = ref(false);
const createError = ref(null);

// Delete user state
const deleteTarget = ref(null);
const showFirstConfirm = ref(false);
const showSecondConfirm = ref(false);
const confirmDeleteName = ref("");

const expandedUsers = reactive({});
const userDetails = reactive({});
const loadingUserDetails = reactive({});

const newRole = reactive({});
const grantPermission = reactive({});
const grantDuration = reactive({});
const grantCountdown = reactive({});

// Password management state
const newPassword = reactive({});
const settingPassword = reactive({});
const passwordError = reactive({});

const availablePermissions = [
  "AccessCatalog",
  "LikeContent",
  "OwnPlaylists",
  "EditCatalog",
  "ManagePermissions",
  "ServerAdmin",
  "ViewAnalytics",
  "RequestContent",
  "DownloadManagerAdmin",
  "UseProxyStreaming",
];

const loadUsers = async () => {
  isLoading.value = true;
  loadError.value = null;

  try {
    const result = await remoteStore.fetchAdminUsers();
    if (result) {
      users.value = result;
    } else {
      loadError.value =
        "Failed to load users. Check browser console for details.";
    }
  } catch (error) {
    console.error("UserManagement: Error loading users:", error);
    loadError.value = `Error: ${error.message || "Unknown error"}`;
  } finally {
    isLoading.value = false;
  }
};

const handleCreateUser = async () => {
  const handle = newUserHandle.value.trim();
  if (!handle || isCreating.value) return;

  isCreating.value = true;
  createError.value = null;

  const result = await remoteStore.createUser(handle);
  if (result.error) {
    createError.value = result.error;
  } else {
    newUserHandle.value = "";
    await loadUsers();
  }

  isCreating.value = false;
};

const initiateDelete = (userHandle) => {
  deleteTarget.value = userHandle;
  showFirstConfirm.value = true;
};

const confirmFirstDelete = () => {
  showFirstConfirm.value = false;
  showSecondConfirm.value = true;
  confirmDeleteName.value = "";
};

const cancelDelete = () => {
  showFirstConfirm.value = false;
  showSecondConfirm.value = false;
  deleteTarget.value = null;
  confirmDeleteName.value = "";
};

const confirmSecondDelete = async () => {
  if (confirmDeleteName.value !== deleteTarget.value) return;

  const result = await remoteStore.deleteUser(deleteTarget.value);
  if (result.error) {
    alert(result.error);
  } else {
    await loadUsers();
  }

  cancelDelete();
};

const toggleUserExpanded = async (userHandle) => {
  if (expandedUsers[userHandle]) {
    expandedUsers[userHandle] = false;
  } else {
    newRole[userHandle] ??= "";
    grantPermission[userHandle] ??= "";
    expandedUsers[userHandle] = true;
    await loadUserDetails(userHandle);
  }
};

const loadUserDetails = async (userHandle) => {
  loadingUserDetails[userHandle] = true;

  const [rolesResult, permissionsResult, credentialsResult] = await Promise.all(
    [
      remoteStore.fetchUserRoles(userHandle),
      remoteStore.fetchUserPermissions(userHandle),
      remoteStore.fetchUserCredentialsStatus(userHandle),
    ],
  );

  userDetails[userHandle] = {
    roles: rolesResult?.roles || [],
    permissions: permissionsResult?.permissions || [],
    hasPassword: credentialsResult?.has_password || false,
  };

  loadingUserDetails[userHandle] = false;
};

const handleAddRole = async (userHandle) => {
  const role = newRole[userHandle];
  if (!role) return;

  const success = await remoteStore.addUserRole(userHandle, role);
  if (success) {
    newRole[userHandle] = "";
    await loadUserDetails(userHandle);
  }
};

const handleRemoveRole = async (userHandle, role) => {
  const success = await remoteStore.removeUserRole(userHandle, role);
  if (success) {
    await loadUserDetails(userHandle);
  }
};

const handleGrantPermission = async (userHandle) => {
  const permission = grantPermission[userHandle];
  if (!permission) return;

  const duration = grantDuration[userHandle] || null;
  const countdown = grantCountdown[userHandle] || null;

  const result = await remoteStore.grantPermission(
    userHandle,
    permission,
    duration,
    countdown,
  );
  if (result) {
    grantPermission[userHandle] = "";
    grantDuration[userHandle] = null;
    grantCountdown[userHandle] = null;
    await loadUserDetails(userHandle);
  }
};

const handleSetPassword = async (userHandle) => {
  const password = newPassword[userHandle];
  if (!password) return;

  settingPassword[userHandle] = true;
  passwordError[userHandle] = null;

  const result = await remoteStore.setUserPassword(userHandle, password);
  if (result.error) {
    passwordError[userHandle] = result.error;
  } else {
    newPassword[userHandle] = "";
    await loadUserDetails(userHandle);
  }

  settingPassword[userHandle] = false;
};

const handleRemovePassword = async (userHandle) => {
  settingPassword[userHandle] = true;
  passwordError[userHandle] = null;

  const result = await remoteStore.deleteUserPassword(userHandle);
  if (result.error) {
    passwordError[userHandle] = result.error;
  } else {
    await loadUserDetails(userHandle);
  }

  settingPassword[userHandle] = false;
};

onMounted(() => {
  loadUsers();
});
</script>

<style scoped>
.userManagement {
  width: 100%;
  color: var(--text-base);
}
.pageHeader {
  margin-bottom: 28px;
}
.sectionTitle {
  margin: 0 0 10px;
  font-size: 32px;
  font-weight: 700;
  letter-spacing: -0.03em;
  line-height: 1.2;
}
.pageHeader p {
  margin: 0;
  font-size: 14px;
  color: var(--text-subdued);
}
.createUserSection {
  display: flex;
  gap: 12px;
  margin-bottom: 28px;
}
input,
select {
  min-width: 0;
  min-height: 44px;
  padding: 10px 12px;
  background: #242424;
  color: var(--text-base);
  color-scheme: dark;
  border: 1px solid #727272;
  border-radius: 4px;
  font: inherit;
  font-size: 14px;
}
input::placeholder {
  color: var(--text-subdued);
}
input:focus-visible,
select:focus-visible {
  outline: 2px solid #fff;
  outline-offset: -2px;
}
.createUserInput {
  flex: 1;
}
button {
  font: inherit;
  font-size: 14px;
  cursor: pointer;
}
button:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 3px;
}
button:disabled {
  opacity: 0.45;
  cursor: default;
}
.createUserButton,
.addButton,
.grantButton,
.setPasswordButton {
  min-height: 44px;
  padding: 0 24px;
  border: 0;
  border-radius: 999px;
  background: var(--spotify-green);
  color: #000;
  font-weight: 700;
  white-space: nowrap;
}
.createUserButton:hover:not(:disabled),
.addButton:hover:not(:disabled),
.grantButton:hover:not(:disabled),
.setPasswordButton:hover:not(:disabled) {
  background: var(--spotify-green-hover);
}
.userList {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.userCard {
  background: #181818;
  border-radius: 8px;
}
.userHeader {
  display: flex;
  align-items: center;
  padding: 4px 12px;
  gap: 12px;
}
.userToggle {
  display: flex;
  align-items: center;
  flex: 1;
  min-width: 0;
  gap: 16px;
  padding: 12px 4px;
  border: 0;
  background: transparent;
  color: var(--text-base);
  text-align: left;
  border-radius: 4px;
}
.userAvatar {
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  flex-shrink: 0;
  border-radius: 50%;
  background: #333;
  color: var(--text-subdued);
  font-size: 18px;
  font-weight: 600;
}
.userName {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
  font-size: 16px;
  font-weight: 600;
}
.expandIcon {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
  fill: none;
  stroke: var(--text-subdued);
  stroke-width: 1.8;
}
.expandIcon.expanded {
  transform: rotate(90deg);
}
.userToggle:hover .userName {
  text-decoration: underline;
}
.deleteUserButton {
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  flex-shrink: 0;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--text-subdued);
}
.deleteUserButton svg {
  width: 20px;
  height: 20px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.6;
}
.deleteUserButton:hover {
  background: #ffffff12;
  color: #f3727f;
}
.userDetails {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 28px;
  padding: 24px;
  border-top: 1px solid var(--surface-border);
}
.detailSection {
  min-width: 0;
}
.detailSection:nth-last-child(2),
.detailSection:last-child {
  grid-column: 1 / -1;
}
.detailTitle {
  margin: 0 0 14px;
  font-size: 16px;
  font-weight: 700;
}
.roleList,
.permissionList {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 12px;
}
.roleTag,
.permissionTag {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  background: #2a2a2a;
  border-radius: 999px;
  font-size: 12px;
  color: var(--text-subdued);
  max-width: 100%;
  overflow-wrap: anywhere;
}
.roleTag {
  color: var(--text-base);
}
.removeButton {
  display: grid;
  place-items: center;
  background: transparent;
  border: 0;
  color: var(--text-subdued);
  width: 24px;
  height: 24px;
  padding: 0;
  font-size: 20px;
  border-radius: 50%;
}
.removeButton:hover {
  color: #fff;
  background: #ffffff12;
}
.addRoleForm,
.grantForm,
.passwordForm {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
}
.permissionSelect {
  flex: 2 1 220px;
  max-width: 100%;
}
.durationInput,
.countdownInput {
  flex: 1 1 160px;
  width: 160px;
}
.passwordInput {
  flex: 1 1 200px;
}
.grantHint {
  margin: 10px 0 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-subdued);
}
.passwordStatus {
  margin-bottom: 12px;
}
.statusBadge,
.emptyState {
  font-size: 13px;
  color: var(--text-subdued);
}
.hasPassword::before {
  content: "";
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--spotify-green);
  margin-right: 8px;
}
.removePasswordButton,
.retryButton {
  min-height: 40px;
  padding: 0 20px;
  border: 1px solid #727272;
  border-radius: 999px;
  background: transparent;
  color: var(--text-base);
  font-weight: 700;
}
.removePasswordButton {
  color: #f3727f;
}
.removePasswordButton:hover,
.retryButton:hover {
  background: #ffffff12;
}
.loadingState,
.errorState,
.emptyUsers,
.detailsLoading {
  padding: 24px;
  color: var(--text-subdued);
  font-size: 14px;
}
.detailsLoading {
  grid-column: 1 / -1;
}
.errorState,
.createError,
.passwordError {
  color: #f3727f;
  font-size: 14px;
  line-height: 1.5;
}
.createError {
  margin-bottom: 16px;
}
.passwordError {
  margin-top: 12px;
}
.dialogOverlay {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  background: #000b;
  z-index: 1000;
}
.dialogBox {
  background: #282828;
  border-radius: 8px;
  padding: 24px;
  width: 440px;
  max-width: 100%;
  max-height: calc(100dvh - 32px);
  overflow-y: auto;
  box-shadow: var(--shadow-menu);
}
.dialogTitle {
  margin: 0 0 20px;
  font-size: 24px;
  font-weight: 700;
  letter-spacing: -0.02em;
}
.dialogMessage {
  margin: 0 0 16px;
  color: var(--text-subdued);
  font-size: 14px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}
.dialogMessage strong {
  color: var(--text-base);
}
.confirmInput {
  width: 100%;
  margin-bottom: 20px;
}
.dialogActions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 12px;
}
.dialogButton {
  min-height: 48px;
  padding: 0 24px;
  border: 0;
  border-radius: 999px;
  font-weight: 700;
}
.cancelButton {
  background: transparent;
  color: var(--text-subdued);
}
.cancelButton:hover {
  color: #fff;
}
.dangerButton {
  background: #f3727f;
  color: #000;
}
.dangerButton:hover:not(:disabled) {
  background: #ff8e99;
}
@media (max-width: 900px) {
  .userDetails {
    grid-template-columns: minmax(0, 1fr);
  }
}
@media (max-width: 480px) {
  .createUserSection {
    flex-direction: column;
  }
  .createUserButton {
    align-self: flex-start;
  }
  .userDetails {
    padding: 20px 16px;
  }
  .grantForm > input,
  .grantForm > select {
    width: 100%;
    flex-basis: 100%;
  }
  .roleSelect {
    max-width: 100%;
  }
}
</style>
