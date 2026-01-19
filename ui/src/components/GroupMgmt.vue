<template>
  <div>
    <!-- Header -->
    <div class="section-header">
      <v-icon icon="mdi-account-group" size="small" color="primary" class="me-3"></v-icon>
      <span class="text-h6 font-weight-bold">{{ editingGroup ? `Edit Group: ${editingGroup}` : 'Group Management' }}</span>
      <span v-if="!editingGroup && items.length > 0" class="group-count">{{ items.length }}</span>
      <v-btn v-if="editingGroup" variant="text" size="small" class="ms-auto" @click="cancelEditGroup">
        <v-icon icon="mdi-arrow-left" size="small" class="me-1"></v-icon>
        Back
      </v-btn>
    </div>

    <!-- Content -->
    <div class="section-content">
      <!-- Groups List View -->
      <template v-if="!editingGroup">
        <p class="text-body-2 text-medium-emphasis mb-5">
          Organize users into groups for easier access control. Groups can be assigned to crates to grant download permissions.
        </p>

        <!-- Groups List -->
        <div v-if="items.length > 0" class="groups-section mb-6">
          <div class="subsection-header">
            <v-icon icon="mdi-folder-account" size="x-small" class="me-2" color="primary"></v-icon>
            <span class="text-body-2 font-weight-medium">Existing Groups</span>
          </div>

          <div class="group-list">
            <div v-for="item in items" :key="item.name" class="group-item">
              <div class="group-info">
                <div class="group-avatar">
                  <v-icon icon="mdi-account-group" size="small"></v-icon>
                </div>
                <span class="group-name">{{ item.name }}</span>
              </div>
              <div class="group-actions">
                <v-btn
                  color="primary"
                  variant="tonal"
                  size="small"
                  @click="editGroup(item.name)"
                >
                  <v-icon icon="mdi-pencil-outline" size="small" class="me-1"></v-icon>
                  Edit
                </v-btn>

                <v-btn
                  color="error"
                  variant="tonal"
                  size="small"
                  @click="promptDeleteGroup(item.name)"
                >
                  <v-icon icon="mdi-delete-outline" size="small"></v-icon>
                </v-btn>
              </div>
            </div>
          </div>
        </div>

        <div v-else class="empty-state mb-6">
          <v-icon icon="mdi-account-group-outline" size="large" class="mb-3 text-medium-emphasis"></v-icon>
          <p class="text-body-2 text-medium-emphasis mb-0">No groups created yet.</p>
        </div>

        <!-- Add Group Form -->
        <div class="add-group-section">
          <div class="subsection-header">
            <v-icon icon="mdi-folder-plus" size="x-small" class="me-2" color="primary"></v-icon>
            <span class="text-body-2 font-weight-medium">Create New Group</span>
          </div>

          <v-form @submit.prevent="addGroup" class="add-group-form">
            <div class="form-row">
              <v-text-field
                v-model="name"
                placeholder="Enter group name"
                prepend-inner-icon="mdi-account-group-outline"
                variant="outlined"
                density="comfortable"
                hide-details
                class="group-input"
              ></v-text-field>
              <v-btn color="primary" type="submit" size="large">
                <v-icon icon="mdi-plus" size="small" class="me-2"></v-icon>
                Create Group
              </v-btn>
            </div>
          </v-form>

          <v-alert
            v-if="addGroupStatus"
            :type="addGroupStatus === 'Success' ? 'success' : 'error'"
            variant="tonal"
            class="mt-4"
            closable
            @click:close="addGroupStatus = ''"
          >
            {{ addGroupMsg }}
          </v-alert>
        </div>
      </template>

      <!-- Edit Group View -->
      <template v-else>
        <!-- Group Members Section -->
        <div class="members-section mb-6">
          <div class="subsection-header">
            <v-icon icon="mdi-account-multiple" size="x-small" class="me-2" color="primary"></v-icon>
            <span class="text-body-2 font-weight-medium">Group Members</span>
            <span class="member-count">{{ groupUsers.length }}</span>
          </div>

          <div v-if="groupUsers.length > 0" class="member-list">
            <div v-for="user in groupUsers" :key="user.name" class="member-item">
              <div class="member-info">
                <div class="member-avatar">
                  <v-icon icon="mdi-account" size="small"></v-icon>
                </div>
                <span class="member-name">{{ user.name }}</span>
              </div>
              <v-btn
                color="error"
                variant="text"
                size="small"
                @click="promptDeleteGroupUser(user.name)"
              >
                <v-icon icon="mdi-close" size="small"></v-icon>
              </v-btn>
            </div>
          </div>

          <div v-else class="empty-state-small">
            <v-icon icon="mdi-account-off" size="small" class="me-2 text-medium-emphasis"></v-icon>
            <span class="text-body-2 text-medium-emphasis">No members in this group yet.</span>
          </div>

          <v-alert
            v-if="deleteUserStatus"
            :type="deleteUserStatus === 'Success' ? 'success' : 'error'"
            variant="tonal"
            class="mt-4"
            closable
            @click:close="deleteUserStatus = ''"
          >
            {{ deleteUserMsg }}
          </v-alert>
        </div>

        <!-- Add Member Section -->
        <div class="add-member-section">
          <div class="subsection-header">
            <v-icon icon="mdi-account-plus" size="x-small" class="me-2" color="primary"></v-icon>
            <span class="text-body-2 font-weight-medium">Add Member</span>
          </div>

          <v-form @submit.prevent="addGroupUser" class="add-member-form">
            <div class="form-row">
              <v-select
                v-model="groupUserName"
                :items="availableUsers"
                item-title="name"
                item-value="name"
                placeholder="Select a user to add"
                prepend-inner-icon="mdi-account-search"
                variant="outlined"
                density="comfortable"
                hide-details
                :disabled="availableUsers.length === 0"
                class="member-select"
              ></v-select>
              <v-btn
                color="primary"
                type="submit"
                size="large"
                :disabled="!groupUserName || availableUsers.length === 0"
              >
                <v-icon icon="mdi-account-plus" size="small" class="me-2"></v-icon>
                Add
              </v-btn>
            </div>

            <p v-if="availableUsers.length === 0" class="text-body-2 text-medium-emphasis mt-3 mb-0">
              <v-icon icon="mdi-information-outline" size="small" class="me-1"></v-icon>
              All users are already members of this group.
            </p>
          </v-form>

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
        </div>
      </template>
    </div>

    <!-- Snackbar for notifications -->
    <v-snackbar
      v-model="showChangeGroupStatus"
      :color="changeGroupStatus === 'Success' ? 'success' : 'error'"
      timeout="5000"
      location="bottom"
    >
      {{ changeGroupMsg }}
      <template v-slot:actions>
        <v-btn variant="text" @click="clearChangeGroupStatus">Close</v-btn>
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
import { onBeforeMount, ref, computed } from "vue";
import {
  ADD_GROUP,
  LIST_GROUPS,
  DELETE_GROUP,
  GROUP_USER,
  GROUP_USERS,
  LIST_USERS,
} from "../remote-routes";
import axios from "axios";
import { useRouter } from "vue-router";

const router = useRouter();
const addGroupStatus = ref("");
const addGroupMsg = ref("");
const addUserStatus = ref("");
const addUserMsg = ref("");
const deleteUserStatus = ref("");
const deleteUserMsg = ref("");
const changeGroupStatus = ref("");
const changeGroupMsg = ref("");
const showChangeGroupStatus = ref(false);
const items = ref([]);
const users = ref([]);
const groupUsers = ref([]);
const groupUserName = ref("");
const name = ref("");
const editingGroup = ref("");

// Confirmation dialog
const confirmDialog = ref(false);
const confirmTitle = ref("");
const confirmMessage = ref("");
const confirmAction = ref(() => { });

const availableUsers = computed(() => {
  return users.value.filter(
    (user) => !groupUsers.value.some((groupUser) => groupUser.name === user.name)
  );
});

onBeforeMount(() => {
  getGroups();
  getUsers();
});

function clearChangeGroupStatus() {
  showChangeGroupStatus.value = false;
  setTimeout(() => {
    changeGroupStatus.value = "";
    changeGroupMsg.value = "";
  }, 300);
}

function editGroup(groupName) {
  editingGroup.value = groupName;
  groupUserName.value = "";
  getGroupUsers();
}

function cancelEditGroup() {
  editingGroup.value = "";
  groupUserName.value = "";
  groupUsers.value = [];
}

function addGroup() {
  const postData = {
    name: name.value,
  };

  axios
    .post(ADD_GROUP, postData)
    .then((res) => {
      if (res.status == 200) {
        addGroupStatus.value = "Success";
        addGroupMsg.value = "Group successfully created.";
        // Clear the form
        name.value = "";
        // Update group list
        getGroups();
      }
    })
    .catch((error) => {
      if (error.response) {
        addGroupStatus.value = "Error";
        addGroupMsg.value = "Group could not be created.";

        if (error.response.status == 404) {
          // "Unauthorized. Login first."
          router.push("/login");
        } else if (error.response.status == 400) {
          addGroupMsg.value = "Invalid group name";
        } else if (error.response.status == 500) {
          addGroupMsg.value = "Group could not be created";
        } else {
          addGroupMsg.value = "Unknown error";
        }
      }
    });
}

function getGroups() {
  axios
    .get(LIST_GROUPS, { cache: false })
    .then((res) => {
      if (res.status == 200) {
        items.value = res.data;
      }
    })
    .catch((error) => {
      console.log(error);
    });
}

function getUsers() {
  axios
    .get(LIST_USERS, { cache: false })
    .then((res) => {
      if (res.status == 200) {
        users.value = res.data;
      }
    })
    .catch((error) => {
      console.log(error);
    });
}

function promptDeleteGroup(name: string) {
  confirmTitle.value = "Delete Group";
  confirmMessage.value = `Are you sure you want to delete group "${name}"? This action cannot be undone.`;
  confirmAction.value = () => deleteGroup(name);
  confirmDialog.value = true;
}

function deleteGroup(name: string) {
  axios
    .delete(DELETE_GROUP(name))
    .then((res) => {
      if (res.status == 200) {
        changeGroupStatus.value = "Success";
        changeGroupMsg.value = `Group "${name}" deleted`;
        showChangeGroupStatus.value = true;
        confirmDialog.value = false;
        getGroups();
      }
    })
    .catch((error) => {
      changeGroupStatus.value = "Error";
      if (error.response.status == 404) {
        // "Unauthorized. Login first."
        router.push("/login");
      } else if (error.response.status == 500) {
        changeGroupMsg.value = "Group could not be deleted";
      } else {
        changeGroupMsg.value = "Unknown error";
      }
      showChangeGroupStatus.value = true;
      confirmDialog.value = false;
    });
}

function addGroupUser() {
  if (!groupUserName.value) return;

  axios
    .put(GROUP_USER(editingGroup.value, groupUserName.value))
    .then((res) => {
      if (res.status == 200) {
        addUserStatus.value = "Success";
        addUserMsg.value = "Member added to group.";
        // Clear selection and update user list
        groupUserName.value = "";
        getGroupUsers();
      }
    })
    .catch((error) => {
      if (error.response) {
        addUserStatus.value = "Error";
        addUserMsg.value = "Member could not be added.";

        if (error.response.status == 404) {
          // "Unauthorized. Login first."
          router.push("/login");
        } else if (error.response.status == 500) {
          addUserMsg.value = "Member could not be added";
        } else {
          addUserMsg.value = "Unknown error";
        }
      }
    });
}

function promptDeleteGroupUser(name: string) {
  confirmTitle.value = "Remove Member";
  confirmMessage.value = `Are you sure you want to remove "${name}" from this group?`;
  confirmAction.value = () => deleteGroupUser(name);
  confirmDialog.value = true;
}

function deleteGroupUser(name: string) {
  axios
    .delete(GROUP_USER(editingGroup.value, name))
    .then((res) => {
      if (res.status == 200) {
        deleteUserStatus.value = "Success";
        deleteUserMsg.value = "Member removed from group.";
        confirmDialog.value = false;
        // Update user list
        getGroupUsers();
      }
    })
    .catch((error) => {
      if (error.response) {
        deleteUserStatus.value = "Error";
        deleteUserMsg.value = "Member could not be removed.";
        confirmDialog.value = false;

        if (error.response.status == 404) {
          // "Unauthorized. Login first."
          router.push("/login");
        } else if (error.response.status == 500) {
          deleteUserMsg.value = "Member could not be removed";
        } else {
          deleteUserMsg.value = "Unknown error";
        }
      }
    });
}

function getGroupUsers() {
  axios
    .get(GROUP_USERS(editingGroup.value), { cache: false })
    .then((res) => {
      if (res.status == 200) {
        groupUsers.value = res.data.users;
      }
    })
    .catch((error) => {
      console.log(error);
    });
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

.group-count {
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

.member-count {
  margin-left: auto;
  background: rgba(var(--v-theme-primary), 0.1);
  color: rgb(var(--v-theme-primary));
  font-size: 11px;
  font-weight: 500;
  padding: 2px 8px;
  border-radius: 10px;
}

/* Group List */
.group-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.group-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
  transition: all 0.2s ease;
}

.group-item:hover {
  border-color: rgb(var(--v-theme-primary));
  background: rgba(var(--v-theme-primary), 0.06);
}

.group-info {
  display: flex;
  align-items: center;
  gap: 12px;
}

.group-avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 8px;
  background: rgba(var(--v-theme-primary), 0.15);
  color: rgb(var(--v-theme-primary));
}

.group-name {
  font-weight: 600;
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface));
}

.group-actions {
  display: flex;
  gap: 8px;
}

/* Member List */
.member-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.member-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 6px;
  transition: all 0.2s ease;
}

.member-item:hover {
  border-color: rgb(var(--v-theme-primary));
  background: rgba(var(--v-theme-primary), 0.03);
}

.member-info {
  display: flex;
  align-items: center;
  gap: 10px;
}

.member-avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 6px;
  background: rgba(var(--v-theme-primary), 0.08);
  color: rgb(var(--v-theme-primary));
}

.member-name {
  font-weight: 500;
  font-size: 14px;
  color: rgb(var(--v-theme-on-surface));
}

/* Empty States */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 32px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px dashed rgb(var(--v-theme-outline));
  border-radius: 8px;
}

.empty-state-small {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  background: rgb(var(--v-theme-surface));
  border: 1px dashed rgb(var(--v-theme-outline));
  border-radius: 6px;
}

/* Add Group/Member Sections */
.add-group-section,
.add-member-section,
.members-section {
  padding: 20px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
}

.add-group-form,
.add-member-form {
  margin-top: 4px;
}

.form-row {
  display: flex;
  gap: 12px;
  align-items: center;
}

.group-input,
.member-select {
  flex: 1;
}

.group-input :deep(.v-field),
.member-select :deep(.v-field) {
  border-radius: 8px;
  background: rgb(var(--v-theme-surface));
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
@media (max-width: 600px) {
  .section-header {
    padding: 16px 20px;
  }

  .section-content {
    padding: 20px;
  }

  .form-row {
    flex-direction: column;
  }

  .group-input,
  .member-select {
    width: 100%;
  }

  .group-item {
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }

  .group-actions {
    width: 100%;
  }
}
</style>
