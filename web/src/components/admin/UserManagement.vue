<template>
  <div class="userManagement">
    <h2 class="sectionTitle">User Management</h2>

    <!-- Create User Section -->
    <div class="createUserSection">
      <input
        v-model="newUserHandle"
        type="text"
        placeholder="New user handle..."
        class="createUserInput"
        @keyup.enter="handleCreateUser"
      />
      <button
        class="createUserButton"
        @click="handleCreateUser"
        :disabled="!newUserHandle.trim() || isCreating"
      >
        {{ isCreating ? "Creating..." : "Create User" }}
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
          <span
            class="userName"
            @click="toggleUserExpanded(user.user_handle)"
            >{{ user.user_handle }}</span
          >
          <div class="userActions">
            <button
              class="deleteUserButton"
              @click.stop="initiateDelete(user.user_handle)"
              title="Delete user"
            >
              ×
            </button>
            <span
              class="expandIcon"
              @click="toggleUserExpanded(user.user_handle)"
              >{{ expandedUsers[user.user_handle] ? "−" : "+" }}</span
            >
          </div>
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
                <select v-model="newRole[user.user_handle]" class="roleSelect">
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
              <h4 class="detailTitle">Current Permissions</h4>
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
              <h4 class="detailTitle">Grant Extra Permission</h4>
              <div class="grantForm">
                <select
                  v-model="grantPermission[user.user_handle]"
                  class="permissionSelect"
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
                  class="durationInput"
                />
                <input
                  v-model.number="grantCountdown[user.user_handle]"
                  type="number"
                  min="0"
                  placeholder="Countdown (uses)"
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

            <!-- Password Login Section -->
            <div class="detailSection">
              <h4 class="detailTitle">Password Login</h4>
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
      <div class="dialogBox">
        <h3 class="dialogTitle">Delete User</h3>
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
      <div class="dialogBox">
        <h3 class="dialogTitle">Confirm Deletion</h3>
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
            Delete Forever
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
  "IssueContentDownload",
  "ServerAdmin",
  "ViewAnalytics",
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
  if (!handle) return;

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
  max-width: 800px;
}

.sectionTitle {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-6) 0;
}

.loadingState,
.errorState,
.emptyUsers {
  padding: var(--spacing-4);
  text-align: center;
  color: var(--text-subdued);
}

.errorState {
  color: #dc2626;
}

.retryButton {
  margin-top: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-4);
  background-color: var(--bg-elevated-base);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  color: var(--text-base);
  cursor: pointer;
}

.userList {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.userCard {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.userHeader {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--spacing-4);
  transition: background-color var(--transition-fast);
}

.userHeader:hover {
  background-color: var(--bg-highlight);
}

.userName {
  font-size: var(--text-lg);
  font-weight: var(--font-medium);
  color: var(--text-base);
  cursor: pointer;
  flex: 1;
}

.expandIcon {
  font-size: var(--text-xl);
  color: var(--text-subdued);
  font-weight: var(--font-bold);
  cursor: pointer;
}

.userDetails {
  padding: 0 var(--spacing-4) var(--spacing-4);
  border-top: 1px solid var(--border-subdued);
}

.detailsLoading {
  padding: var(--spacing-4);
  text-align: center;
  color: var(--text-subdued);
}

.detailSection {
  margin-top: var(--spacing-4);
}

.detailTitle {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--text-subdued);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin: 0 0 var(--spacing-2) 0;
}

.roleList,
.permissionList {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-2);
  margin-bottom: var(--spacing-2);
}

.roleTag,
.permissionTag {
  display: inline-flex;
  align-items: center;
  gap: var(--spacing-1);
  padding: var(--spacing-1) var(--spacing-3);
  background-color: var(--bg-highlight);
  border-radius: var(--radius-full);
  font-size: var(--text-sm);
  color: var(--text-base);
}

.roleTag {
  background-color: var(--spotify-green);
  color: white;
}

.removeButton {
  background: none;
  border: none;
  color: inherit;
  font-size: var(--text-lg);
  cursor: pointer;
  padding: 0;
  margin-left: var(--spacing-1);
  opacity: 0.7;
  line-height: 1;
}

.removeButton:hover {
  opacity: 1;
}

.emptyState {
  color: var(--text-subdued);
  font-style: italic;
  font-size: var(--text-sm);
}

.addRoleForm,
.grantForm {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-2);
  align-items: center;
}

.roleSelect,
.permissionSelect,
.durationInput,
.countdownInput {
  padding: var(--spacing-2) var(--spacing-3);
  background-color: var(--bg-base);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  color: var(--text-base);
  font-size: var(--text-sm);
}

.durationInput,
.countdownInput {
  width: 140px;
}

.addButton,
.grantButton {
  padding: var(--spacing-2) var(--spacing-4);
  background-color: var(--spotify-green);
  color: white;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.addButton:hover,
.grantButton:hover {
  background-color: #1ed760;
}

.grantHint {
  font-size: var(--text-xs);
  color: var(--text-subdued);
  margin: var(--spacing-2) 0 0 0;
}

/* Create User Section */
.createUserSection {
  display: flex;
  gap: var(--spacing-2);
  margin-bottom: var(--spacing-4);
}

.createUserInput {
  flex: 1;
  padding: var(--spacing-3) var(--spacing-4);
  background-color: var(--bg-elevated-base);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  color: var(--text-base);
  font-size: var(--text-base);
}

.createUserInput::placeholder {
  color: var(--text-subdued);
}

.createUserButton {
  padding: var(--spacing-3) var(--spacing-6);
  background-color: var(--spotify-green);
  color: white;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-base);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.createUserButton:hover:not(:disabled) {
  background-color: #1ed760;
}

.createUserButton:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.createError {
  color: #dc2626;
  font-size: var(--text-sm);
  margin-bottom: var(--spacing-4);
}

/* User Actions */
.userActions {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.deleteUserButton {
  background: none;
  border: none;
  color: var(--text-subdued);
  font-size: var(--text-xl);
  cursor: pointer;
  padding: var(--spacing-1) var(--spacing-2);
  border-radius: var(--radius-md);
  transition: all var(--transition-fast);
  line-height: 1;
}

.deleteUserButton:hover {
  background-color: rgba(220, 38, 38, 0.2);
  color: #dc2626;
}

/* Dialog Styles */
.dialogOverlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.dialogBox {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  padding: var(--spacing-6);
  max-width: 400px;
  width: 90%;
}

.dialogTitle {
  font-size: var(--text-xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-4) 0;
}

.dialogMessage {
  font-size: var(--text-base);
  color: var(--text-subdued);
  margin: 0 0 var(--spacing-3) 0;
  line-height: 1.5;
}

.dialogMessage strong {
  color: var(--text-base);
}

.confirmInput {
  width: 100%;
  padding: var(--spacing-3);
  background-color: var(--bg-base);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  color: var(--text-base);
  font-size: var(--text-base);
  margin-bottom: var(--spacing-4);
}

.dialogActions {
  display: flex;
  gap: var(--spacing-3);
  justify-content: flex-end;
}

.dialogButton {
  padding: var(--spacing-2) var(--spacing-4);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.cancelButton {
  background-color: var(--bg-highlight);
  border: 1px solid var(--border-default);
  color: var(--text-base);
}

.cancelButton:hover {
  background-color: var(--bg-base);
}

.dangerButton {
  background-color: #dc2626;
  border: none;
  color: white;
}

.dangerButton:hover:not(:disabled) {
  background-color: #b91c1c;
}

.dangerButton:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Password Section */
.passwordStatus {
  margin-bottom: var(--spacing-2);
}

.statusBadge {
  display: inline-block;
  padding: var(--spacing-1) var(--spacing-3);
  border-radius: var(--radius-full);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
}

.hasPassword {
  background-color: var(--spotify-green);
  color: white;
}

.noPassword {
  background-color: var(--bg-highlight);
  color: var(--text-subdued);
}

.passwordForm {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-2);
  align-items: center;
}

.passwordInput {
  flex: 1;
  min-width: 150px;
  padding: var(--spacing-2) var(--spacing-3);
  background-color: var(--bg-base);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  color: var(--text-base);
  font-size: var(--text-sm);
}

.setPasswordButton {
  padding: var(--spacing-2) var(--spacing-4);
  background-color: var(--spotify-green);
  color: white;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.setPasswordButton:hover:not(:disabled) {
  background-color: #1ed760;
}

.setPasswordButton:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.removePasswordButton {
  padding: var(--spacing-2) var(--spacing-4);
  background-color: transparent;
  color: #dc2626;
  border: 1px solid #dc2626;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.removePasswordButton:hover:not(:disabled) {
  background-color: rgba(220, 38, 38, 0.1);
}

.removePasswordButton:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.passwordError {
  color: #dc2626;
  font-size: var(--text-sm);
  margin-top: var(--spacing-2);
}
</style>
