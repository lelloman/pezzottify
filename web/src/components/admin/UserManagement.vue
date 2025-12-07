<template>
  <div class="userManagement">
    <h2 class="sectionTitle">User Management</h2>

    <div v-if="isLoading" class="loadingState">
      Loading users...
    </div>

    <div v-else-if="loadError" class="errorState">
      {{ loadError }}
      <button class="retryButton" @click="loadUsers">Retry</button>
    </div>

    <div v-else class="userList">
      <div
        v-for="user in users"
        :key="user.user_handle"
        class="userCard"
      >
        <div class="userHeader" @click="toggleUserExpanded(user.user_handle)">
          <span class="userName">{{ user.user_handle }}</span>
          <span class="expandIcon">{{ expandedUsers[user.user_handle] ? '−' : '+' }}</span>
        </div>

        <div v-if="expandedUsers[user.user_handle]" class="userDetails">
          <!-- Loading state for user details -->
          <div v-if="loadingUserDetails[user.user_handle]" class="detailsLoading">
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
                  <button class="removeButton" @click="handleRemoveRole(user.user_handle, role)" title="Remove role">×</button>
                </span>
                <span v-if="!userDetails[user.user_handle]?.roles?.length" class="emptyState">No roles</span>
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
                  v-for="perm in userDetails[user.user_handle]?.permissions || []"
                  :key="perm"
                  class="permissionTag"
                >
                  {{ perm }}
                </span>
                <span v-if="!userDetails[user.user_handle]?.permissions?.length" class="emptyState">No permissions</span>
              </div>
            </div>

            <!-- Grant Permission Section -->
            <div class="detailSection">
              <h4 class="detailTitle">Grant Extra Permission</h4>
              <div class="grantForm">
                <select v-model="grantPermission[user.user_handle]" class="permissionSelect">
                  <option value="">Select permission...</option>
                  <option v-for="perm in availablePermissions" :key="perm" :value="perm">{{ perm }}</option>
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
              <p class="grantHint">Leave duration and countdown empty for permanent permission.</p>
            </div>
          </template>
        </div>
      </div>

      <div v-if="users.length === 0" class="emptyUsers">
        No users found.
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted } from 'vue';
import { useRemoteStore } from '@/store/remote';

const remoteStore = useRemoteStore();

const users = ref([]);
const isLoading = ref(true);
const loadError = ref(null);

const expandedUsers = reactive({});
const userDetails = reactive({});
const loadingUserDetails = reactive({});

const newRole = reactive({});
const grantPermission = reactive({});
const grantDuration = reactive({});
const grantCountdown = reactive({});

const availablePermissions = [
  'AccessCatalog',
  'LikeContent',
  'OwnPlaylists',
  'EditCatalog',
  'ManagePermissions',
  'IssueContentDownload',
  'RebootServer',
  'ViewAnalytics',
];

const loadUsers = async () => {
  isLoading.value = true;
  loadError.value = null;

  try {
    const result = await remoteStore.fetchAdminUsers();
    if (result) {
      users.value = result;
    } else {
      loadError.value = 'Failed to load users. Check browser console for details.';
    }
  } catch (error) {
    console.error('UserManagement: Error loading users:', error);
    loadError.value = `Error: ${error.message || 'Unknown error'}`;
  } finally {
    isLoading.value = false;
  }
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

  const [rolesResult, permissionsResult] = await Promise.all([
    remoteStore.fetchUserRoles(userHandle),
    remoteStore.fetchUserPermissions(userHandle),
  ]);

  userDetails[userHandle] = {
    roles: rolesResult?.roles || [],
    permissions: permissionsResult?.permissions || [],
  };

  loadingUserDetails[userHandle] = false;
};

const handleAddRole = async (userHandle) => {
  const role = newRole[userHandle];
  if (!role) return;

  const success = await remoteStore.addUserRole(userHandle, role);
  if (success) {
    newRole[userHandle] = '';
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

  const result = await remoteStore.grantPermission(userHandle, permission, duration, countdown);
  if (result) {
    grantPermission[userHandle] = '';
    grantDuration[userHandle] = null;
    grantCountdown[userHandle] = null;
    await loadUserDetails(userHandle);
  }
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
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.userHeader:hover {
  background-color: var(--bg-highlight);
}

.userName {
  font-size: var(--text-lg);
  font-weight: var(--font-medium);
  color: var(--text-base);
}

.expandIcon {
  font-size: var(--text-xl);
  color: var(--text-subdued);
  font-weight: var(--font-bold);
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
</style>
