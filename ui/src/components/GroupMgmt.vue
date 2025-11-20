<template>
  <v-container>
    <!-- Groups List -->
    <v-card class="mb-6 pa-4">
      <v-card-title class="text-h4 pb-2">Groups</v-card-title>

      <div v-if="items.length === 0" class="text-center pa-4">
        <v-icon icon="mdi-account-group" size="large" color="grey-lighten-1" class="mb-2"></v-icon>
        <div class="text-body-1 text-grey">No groups found. Create a new group below.</div>
      </div>

      <v-card v-for="item in items" :key="item.name" class="mb-3 pa-3">
        <v-row align="center">
          <v-col cols="12" sm="6">
            <div class="text-subtitle-1 font-weight-bold d-flex align-center">
              <v-icon icon="mdi-account-group" class="mr-2"></v-icon>
              {{ item.name }}
            </div>
          </v-col>

          <v-col cols="12" sm="6" class="d-flex flex-wrap gap-2 justify-end">
            <v-btn color="warning" variant="outlined" size="small" @click="editGroup(item.name)">
              <v-icon start>mdi-pencil</v-icon>
              Edit
            </v-btn>

            <v-btn color="error" variant="outlined" size="small" @click="promptDeleteGroup(item.name)">
              <v-icon start>mdi-delete</v-icon>
              Delete
            </v-btn>
          </v-col>
        </v-row>
      </v-card>

      <v-snackbar v-model="showChangeGroupStatus" :color="changeGroupStatus === 'Success' ? 'success' : 'error'"
        timeout="5000">
        {{ changeGroupMsg }}
        <template v-slot:actions>
          <v-btn variant="text" @click="clearChangeGroupStatus">Close</v-btn>
        </template>
      </v-snackbar>
    </v-card>

    <!-- Add Group Section -->
    <v-card v-if="!editingGroup" class="pa-4">
      <v-card-title class="text-h5 pb-2">Add Group</v-card-title>
      <v-form @submit.prevent="addGroup">
        <v-text-field v-model="name" label="Group Name" prepend-inner-icon="mdi-account-group" variant="outlined"
          class="mb-2"></v-text-field>

        <v-alert v-if="addGroupStatus" :type="addGroupStatus === 'Success' ? 'success' : 'error'" variant="tonal"
          class="my-4" closable @click:close="addGroupStatus = ''">
          {{ addGroupMsg }}
        </v-alert>

        <v-btn type="submit" color="primary" class="mt-2">
          <v-icon start>mdi-plus</v-icon>
          Add Group
        </v-btn>
      </v-form>
    </v-card>

    <!-- Edit Group Section -->
    <v-card v-else class="pa-4">
      <v-card-title class="text-h5 pb-2">
        <v-icon icon="mdi-account-group" class="mr-2"></v-icon>
        Edit Group: {{ editingGroup }}
      </v-card-title>

      <!-- Group Users List -->
      <v-card class="mb-4 pa-3 bg-grey-lighten-5">
        <v-card-title class="text-h6">Group Members</v-card-title>

        <div v-if="groupUsers.length === 0" class="text-center pa-4">
          <v-icon icon="mdi-account-off" size="large" color="grey-lighten-1" class="mb-2"></v-icon>
          <div class="text-body-1 text-grey">No users in this group. Add members below.</div>
        </div>

        <v-list v-else>
          <v-list-item v-for="user in groupUsers" :key="user.name" class="py-1">
            <template v-slot:prepend>
              <v-avatar color="primary" size="32">
                <v-icon icon="mdi-account" size="small" color="white"></v-icon>
              </v-avatar>
            </template>

            <v-list-item-title>{{ user.name }}</v-list-item-title>

            <template v-slot:append>
              <v-btn variant="text" color="error" density="comfortable" icon="mdi-delete"
                @click="promptDeleteGroupUser(user.name)"></v-btn>
            </template>
          </v-list-item>
        </v-list>

        <v-alert v-if="deleteUserStatus" :type="deleteUserStatus === 'Success' ? 'success' : 'error'" variant="tonal"
          class="mt-4" closable @click:close="deleteUserStatus = ''">
          {{ deleteUserMsg }}
        </v-alert>
      </v-card>

      <!-- Add User to Group -->
      <v-card class="mb-4 pa-3 bg-grey-lighten-5">
        <v-card-title class="text-h6">Add Member</v-card-title>

        <v-form @submit.prevent="addGroupUser" class="mt-2">
          <v-row>
            <v-col cols="12" sm="8">
              <v-select v-model="groupUserName" :items="availableUsers" item-title="name" item-value="name"
                label="Select User" variant="outlined" prepend-inner-icon="mdi-account-plus"
                :disabled="availableUsers.length === 0"
                :hint="availableUsers.length === 0 ? 'No available users to add' : ''" persistent-hint></v-select>
            </v-col>

            <v-col cols="12" sm="4" class="d-flex align-center">
              <v-btn type="submit" color="primary" :disabled="!groupUserName || availableUsers.length === 0" block>
                <v-icon start>mdi-account-plus</v-icon>
                Add User
              </v-btn>
            </v-col>
          </v-row>
        </v-form>

        <v-alert v-if="addUserStatus" :type="addUserStatus === 'Success' ? 'success' : 'error'" variant="tonal"
          class="mt-4" closable @click:close="addUserStatus = ''">
          {{ addUserMsg }}
        </v-alert>
      </v-card>

      <v-btn color="grey" variant="outlined" class="mt-2" @click="cancelEditGroup">
        <v-icon start>mdi-arrow-left</v-icon>
        Back to Groups
      </v-btn>
    </v-card>

    <!-- Confirmation Dialogs -->
    <v-dialog v-model="confirmDialog" max-width="500">
      <v-card>
        <v-card-title>{{ confirmTitle }}</v-card-title>
        <v-card-text>{{ confirmMessage }}</v-card-text>
        <v-card-actions>
          <v-spacer></v-spacer>
          <v-btn color="grey-darken-1" variant="text" @click="confirmDialog = false">Cancel</v-btn>
          <v-btn color="primary" variant="text" @click="confirmAction">Confirm</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </v-container>
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
        addGroupMsg.value = "Group successfully added.";
        // Clear the form
        name.value = "";
        // Update group list
        getGroups();
      }
    })
    .catch((error) => {
      if (error.response) {
        addGroupStatus.value = "Error";
        addGroupMsg.value = "Group could not be added.";

        if (error.response.status == 404) {
          // "Unauthorized. Login first."
          router.push("/login");
        } else if (error.response.status == 400) {
          addGroupMsg.value = "Invalid Name";
        } else if (error.response.status == 500) {
          addGroupMsg.value = "Group could not be added";
        } else {
          addGroupMsg.value = "Unknown error";
        }
      }
    });
}

function getGroups() {
  axios
    .get(LIST_GROUPS, { cache: false }) // disable caching to get updated token list
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
    .get(LIST_USERS, { cache: false }) // disable caching to get updated token list
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
  confirmMessage.value = `Are you sure you want to delete group "${name}"?`;
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
        addUserMsg.value = "Group user successfully added.";
        // Clear selection and update user list
        groupUserName.value = "";
        getGroupUsers();
      }
    })
    .catch((error) => {
      if (error.response) {
        addUserStatus.value = "Error";
        addUserMsg.value = "Group user could not be added.";

        if (error.response.status == 404) {
          // "Unauthorized. Login first."
          router.push("/login");
        } else if (error.response.status == 500) {
          addUserMsg.value = "Group user could not be added";
        } else {
          addUserMsg.value = "Unknown error";
        }
      }
    });
}

function promptDeleteGroupUser(name: string) {
  confirmTitle.value = "Remove User from Group";
  confirmMessage.value = `Are you sure you want to remove user "${name}" from the group?`;
  confirmAction.value = () => deleteGroupUser(name);
  confirmDialog.value = true;
}

function deleteGroupUser(name: string) {
  axios
    .delete(GROUP_USER(editingGroup.value, name))
    .then((res) => {
      if (res.status == 200) {
        deleteUserStatus.value = "Success";
        deleteUserMsg.value = "Group user successfully removed.";
        confirmDialog.value = false;
        // Update user list
        getGroupUsers();
      }
    })
    .catch((error) => {
      if (error.response) {
        deleteUserStatus.value = "Error";
        deleteUserMsg.value = "Group user could not be removed.";
        confirmDialog.value = false;

        if (error.response.status == 404) {
          // "Unauthorized. Login first."
          router.push("/login");
        } else if (error.response.status == 500) {
          deleteUserMsg.value = "Group user could not be removed";
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
.gap-2 {
  gap: 8px;
}
</style>
