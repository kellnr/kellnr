<template>
  <div>
    <!-- Header -->
    <div class="section-header">
      <v-icon icon="mdi-account-multiple" size="small" color="primary" class="me-3"></v-icon>
      <span class="text-h6 font-weight-bold">User Management</span>
      <span v-if="items.length > 0" class="user-count">{{ items.length }}</span>
    </div>

    <!-- Content -->
    <div class="section-content">
      <p class="text-body-2 text-medium-emphasis mb-5">
        Manage user accounts, permissions, and access levels. Admins have full access, while read-only users can only view crates.
      </p>

      <!-- User List -->
      <div v-if="items.length > 0" class="users-section mb-6">
        <div class="subsection-header">
          <v-icon icon="mdi-account-group" size="x-small" class="me-2" color="primary"></v-icon>
          <span class="text-body-2 font-weight-medium">Registered Users</span>
        </div>

        <div class="user-list">
          <div v-for="item in items" :key="item.name" class="user-item">
            <div class="user-info">
              <div class="user-avatar" :class="{ admin: item.is_admin }">
                <v-icon :icon="item.is_admin ? 'mdi-shield-account' : 'mdi-account'" size="small"></v-icon>
              </div>
              <div class="user-details">
                <span class="user-name">{{ item.name }}</span>
                <div class="user-badges">
                  <span class="role-badge" :class="item.is_admin ? 'admin' : 'user'">
                    {{ item.is_admin ? 'Admin' : 'User' }}
                  </span>
                  <span v-if="item.is_read_only" class="role-badge readonly">
                    Read-only
                  </span>
                </div>
              </div>
            </div>
            <div class="user-actions">
              <v-btn
                :color="item.is_read_only ? 'info' : 'default'"
                variant="tonal"
                size="small"
                @click="set_read_only(item.name, !item.is_read_only, item)"
              >
                <v-icon :icon="item.is_read_only ? 'mdi-lock-open-outline' : 'mdi-lock-outline'" size="small" class="me-1"></v-icon>
                {{ item.is_read_only ? 'Unlock' : 'Lock' }}
              </v-btn>

              <v-btn
                color="warning"
                variant="tonal"
                size="small"
                @click="resetPwd(item.name)"
              >
                <v-icon icon="mdi-key-outline" size="small" class="me-1"></v-icon>
                Reset
              </v-btn>

              <v-btn
                color="error"
                variant="tonal"
                size="small"
                @click="deleteUser(item.name)"
              >
                <v-icon icon="mdi-delete-outline" size="small"></v-icon>
              </v-btn>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="empty-state mb-6">
        <v-icon icon="mdi-account-off" size="large" class="mb-3 text-medium-emphasis"></v-icon>
        <p class="text-body-2 text-medium-emphasis mb-0">No users registered yet.</p>
      </div>

      <!-- Add User Form -->
      <div class="add-user-section">
        <div class="subsection-header">
          <v-icon icon="mdi-account-plus" size="x-small" class="me-2" color="primary"></v-icon>
          <span class="text-body-2 font-weight-medium">Add New User</span>
        </div>

        <v-form @submit.prevent="addUser" class="add-user-form">
          <div class="form-grid">
            <div class="form-field">
              <label class="field-label">Username</label>
              <v-text-field
                v-model="name"
                placeholder="Enter username"
                prepend-inner-icon="mdi-account-outline"
                variant="outlined"
                density="comfortable"
                hide-details
              ></v-text-field>
            </div>

            <div class="form-field">
              <label class="field-label">Password</label>
              <v-text-field
                v-model="pwd1"
                placeholder="Enter password"
                prepend-inner-icon="mdi-lock-outline"
                type="password"
                variant="outlined"
                density="comfortable"
                hide-details
              ></v-text-field>
            </div>

            <div class="form-field">
              <label class="field-label">Confirm Password</label>
              <v-text-field
                v-model="pwd2"
                placeholder="Confirm password"
                prepend-inner-icon="mdi-lock-check-outline"
                type="password"
                variant="outlined"
                density="comfortable"
                hide-details
              ></v-text-field>
            </div>
          </div>

          <div class="form-options">
            <v-checkbox
              v-model="is_admin"
              label="Administrator"
              hide-details
              density="compact"
              class="option-checkbox"
            >
              <template v-slot:label>
                <div class="checkbox-label">
                  <span class="checkbox-title">Administrator</span>
                  <span class="checkbox-desc">Full access to all features</span>
                </div>
              </template>
            </v-checkbox>

            <v-checkbox
              v-model="is_read_only"
              label="Read-only"
              hide-details
              density="compact"
              class="option-checkbox"
            >
              <template v-slot:label>
                <div class="checkbox-label">
                  <span class="checkbox-title">Read-only</span>
                  <span class="checkbox-desc">Can only view crates</span>
                </div>
              </template>
            </v-checkbox>
          </div>

          <v-alert
            v-if="addUserStatus"
            :type="addUserStatus === 'Success' ? 'success' : 'error'"
            variant="tonal"
            class="mt-4"
            closable
            @click:close="addUserStatus = ''"
          >
            {{ addUserMsg }}
          </v-alert>

          <div class="form-actions">
            <v-btn type="submit" color="primary" size="large">
              <v-icon icon="mdi-account-plus" size="small" class="me-2"></v-icon>
              Create User
            </v-btn>
          </div>
        </v-form>
      </div>
    </div>

    <!-- Snackbar for notifications -->
    <v-snackbar
      v-model="showChangeUserStatus"
      :color="changeUserStatus === 'Success' ? 'success' : 'error'"
      timeout="5000"
      location="bottom"
    >
      {{ changeUserMsg }}
      <template v-slot:actions>
        <v-btn variant="text" @click="clearChangeUserStatus">Close</v-btn>
      </template>
    </v-snackbar>

    <!-- Confirmation Dialog -->
    <v-dialog v-model="confirmDialog" max-width="450">
      <v-card class="confirm-dialog">
        <div class="dialog-header">
          <v-icon icon="mdi-alert-circle" color="warning" size="small" class="me-3"></v-icon>
          <span class="text-h6 font-weight-bold">{{ confirmTitle }}</span>
        </div>

        <v-card-text class="pa-5">
          <p class="text-body-1 mb-0">{{ confirmMessage }}</p>
        </v-card-text>

        <v-card-actions class="pa-4 pt-0">
          <v-spacer></v-spacer>
          <v-btn variant="text" @click="confirmDialog = false">Cancel</v-btn>
          <v-btn color="primary" variant="flat" @click="confirmAction">Confirm</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, ref } from 'vue'
import { ADD_USER, DELETE_USER, LIST_USERS, RESET_PWD, USER_READ_ONLY } from "../remote-routes";
import axios from "axios";
import { useRouter } from "vue-router";

const router = useRouter();
const addUserStatus = ref("")
const addUserMsg = ref("")
const changeUserStatus = ref("")
const changeUserMsg = ref("")
const showChangeUserStatus = ref(false)
const items = ref([])
const name = ref("")
const pwd1 = ref("")
const pwd2 = ref("")
const is_admin = ref(false)
const is_read_only = ref(false)

// Confirmation dialog
const confirmDialog = ref(false)
const confirmTitle = ref("")
const confirmMessage = ref("")
const confirmAction = ref(() => { })

onBeforeMount(() => {
  getUsers()
})

function clearChangeUserStatus() {
  showChangeUserStatus.value = false
  setTimeout(() => {
    changeUserStatus.value = ""
    changeUserMsg.value = ""
  }, 300)
}

function addUser() {
  const postData = {
    name: name.value,
    pwd1: pwd1.value,
    pwd2: pwd2.value,
    is_admin: is_admin.value,
    is_read_only: is_read_only.value
  };

  axios
    .post(ADD_USER, postData)
    .then((res) => {
      if (res.status == 200) {
        addUserStatus.value = "Success";
        addUserMsg.value = "User successfully added.";
        // Clear form
        name.value = "";
        pwd1.value = "";
        pwd2.value = "";
        is_admin.value = false;
        is_read_only.value = false;
        // Update user list
        getUsers();
      }
    })
    .catch((error) => {
      if (error.response) {
        addUserStatus.value = "Error";
        addUserMsg.value = "User could not be added.";

        if (error.response.status == 404) {
          // "Unauthorized. Login first."
          router.push("/login");
        } else if (error.response.status == 400) {
          addUserMsg.value = "Passwords do not match";
        } else if (error.response.status == 500) {
          addUserMsg.value = "User could not be added";
        } else {
          addUserMsg.value = "Unknown error";
        }
      }
    });
}

function getUsers() {
  axios
    .get(LIST_USERS, { cache: false })
    .then((res) => {
      if (res.status == 200) {
        items.value = res.data;
      }
    })
    .catch((error) => {
      console.log(error);
    });
}

function deleteUser(name: string) {
  confirmTitle.value = "Delete User";
  confirmMessage.value = `Are you sure you want to delete user "${name}"? This action cannot be undone.`;
  confirmAction.value = () => {
    axios
      .delete(DELETE_USER(name))
      .then((res) => {
        if (res.status == 200) {
          changeUserStatus.value = "Success";
          changeUserMsg.value = `User "${name}" deleted`;
          showChangeUserStatus.value = true;
          confirmDialog.value = false;
          getUsers();
        }
      })
      .catch((error) => {
        changeUserStatus.value = "Error";
        if (error.response.status == 404) {
          // "Unauthorized. Login first."
          router.push("/login");
        } else if (error.response.status == 500) {
          changeUserMsg.value = "User could not be deleted";
        } else {
          changeUserMsg.value = "Unknown error";
        }
        showChangeUserStatus.value = true;
        confirmDialog.value = false;
      });
  };
  confirmDialog.value = true;
}

function resetPwd(name: string) {
  confirmTitle.value = "Reset Password";
  confirmMessage.value = `Are you sure you want to reset the password for "${name}"?`;
  confirmAction.value = () => {
    axios
      .post(RESET_PWD(name))
      .then((res) => {
        if (res.status == 200) {
          changeUserStatus.value = "Success";
          changeUserMsg.value =
            `Password for "${name}" reset to "${res.data["new_pwd"]}".
            Notify the user to change the password on the next login.`;
          showChangeUserStatus.value = true;
          confirmDialog.value = false;
        }
      })
      .catch((error) => {
        changeUserStatus.value = "Error";
        if (error.response.status == 404) {
          // "Unauthorized. Login first."
          router.push("/login");
        } else if (error.response.status == 500) {
          changeUserMsg.value = "Password could not be reset";
        } else {
          changeUserMsg.value = "Unknown error";
        }
        showChangeUserStatus.value = true;
        confirmDialog.value = false;
      });
  };
  confirmDialog.value = true;
}

function set_read_only(name: string, state: boolean, item: any) {
  confirmTitle.value = "Change Read-only Status";
  confirmMessage.value = state
    ? `Make "${name}" read-only? They will only be able to view crates.`
    : `Remove read-only restriction from "${name}"?`;

  confirmAction.value = () => {
    axios
      .post(USER_READ_ONLY(name), { state: state })
      .then((res) => {
        if (res.status == 200) {
          changeUserStatus.value = "Success";
          if (state) {
            changeUserMsg.value = `"${name}" was made read-only`;
            item.is_read_only = true; // update UI
          } else {
            changeUserMsg.value = `Removed read-only from "${name}"`;
            item.is_read_only = false; // update UI
          }
          showChangeUserStatus.value = true;
          confirmDialog.value = false;
        }
      })
      .catch((error) => {
        changeUserStatus.value = "Error";
        if (error.response.status == 404) {
          // "Unauthorized. Login first."
          router.push("/login");
        } else if (error.response.status == 500) {
          changeUserMsg.value = "Read-only status could not be modified";
        } else {
          changeUserMsg.value = "Unknown error";
        }
        showChangeUserStatus.value = true;
        confirmDialog.value = false;
      });
  };
  confirmDialog.value = true;
}
</script>

<style scoped>
.section-header {
  display: flex;
  align-items: center;
  padding: 16px 24px;
  background: rgba(var(--v-theme-primary), 0.05);
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.user-count {
  margin-left: auto;
  background: rgb(var(--v-theme-primary));
  color: rgb(var(--v-theme-on-primary));
  font-size: 12px;
  font-weight: 600;
  padding: 2px 10px;
  border-radius: 12px;
}

.section-content {
  padding: 24px;
}

/* Subsection Header */
.subsection-header {
  display: flex;
  align-items: center;
  margin-bottom: 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

/* User List */
.user-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.user-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  transition: all 0.2s ease;
}

.user-item:hover {
  border-color: rgb(var(--v-theme-primary));
  background: rgba(var(--v-theme-primary), 0.06);
}

.user-info {
  display: flex;
  align-items: center;
  gap: 14px;
}

.user-avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border-radius: 10px;
  background: rgba(var(--v-theme-primary), 0.08);
  color: rgb(var(--v-theme-primary));
}

.user-avatar.admin {
  background: rgba(var(--v-theme-primary), 0.15);
  color: rgb(var(--v-theme-primary));
}

.user-details {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.user-name {
  font-weight: 600;
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface));
}

.user-badges {
  display: flex;
  gap: 6px;
}

.role-badge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.3px;
}

.role-badge.admin {
  background: rgba(var(--v-theme-primary), 0.15);
  color: rgb(var(--v-theme-primary));
}

.role-badge.user {
  background: rgba(var(--v-theme-info), 0.15);
  color: rgb(var(--v-theme-info));
}

.role-badge.readonly {
  background: rgba(var(--v-theme-warning), 0.15);
  color: rgb(var(--v-theme-warning));
}

.user-actions {
  display: flex;
  gap: 8px;
}

/* Empty State */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 32px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px dashed rgb(var(--v-theme-outline));
  border-radius: 8px;
}

/* Add User Section */
.add-user-section {
  padding: 20px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
}

.add-user-form {
  margin-top: 4px;
}

.form-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 16px;
  margin-bottom: 20px;
}

.form-field {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.field-label {
  font-size: 13px;
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface));
}

.form-field :deep(.v-field) {
  border-radius: 8px;
  background: rgb(var(--v-theme-surface));
}

.form-options {
  display: flex;
  gap: 24px;
  padding: 16px;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  margin-bottom: 16px;
}

.option-checkbox {
  flex: 1;
}

.checkbox-label {
  display: flex;
  flex-direction: column;
}

.checkbox-title {
  font-size: 14px;
  font-weight: 500;
  color: rgb(var(--v-theme-on-surface));
}

.checkbox-desc {
  font-size: 12px;
  color: rgb(var(--v-theme-on-surface-variant));
}

.form-actions {
  padding-top: 16px;
  border-top: 1px solid rgb(var(--v-theme-outline));
}

/* Confirm Dialog */
.confirm-dialog {
  border-radius: 12px;
  overflow: hidden;
}

.dialog-header {
  display: flex;
  align-items: center;
  padding: 16px 20px;
  background: rgba(var(--v-theme-warning), 0.08);
  border-bottom: 1px solid rgba(var(--v-theme-warning), 0.2);
}

/* Responsive */
@media (max-width: 768px) {
  .user-item {
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }

  .user-actions {
    width: 100%;
    justify-content: flex-start;
  }

  .form-options {
    flex-direction: column;
    gap: 12px;
  }
}

@media (max-width: 600px) {
  .section-header {
    padding: 16px 20px;
  }

  .section-content {
    padding: 20px;
  }

  .form-grid {
    grid-template-columns: 1fr;
  }
}
</style>
