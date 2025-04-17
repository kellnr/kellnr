<template>
  <v-container>
    <v-card-title class="text-h4 pb-2">Users</v-card-title>

    <v-card v-for="item in items" :key="item.name" class="mb-3 pa-3">
      <v-row align="center">
        <v-col cols="12" sm="4">
          <div class="text-subtitle-1 font-weight-bold">{{ item.name }}</div>
          <v-chip :color="item.is_admin ? 'primary' : 'info'" size="small" class="mt-1">
            {{ item.is_admin ? 'Admin' : 'User' }}
          </v-chip>
        </v-col>

        <v-col cols="12" sm="8" class="d-flex flex-wrap gap-2 justify-end">
          <v-btn :color="item.is_read_only ? 'info' : 'primary'" variant="outlined" size="small"
            @click="set_read_only(item.name, !item.is_read_only, item)">
            <v-icon start>{{ item.is_read_only ? 'mdi-lock-open' : 'mdi-lock' }}</v-icon>
            {{ item.is_read_only ? 'Remove Read-only' : 'Make Read-only' }}
          </v-btn>

          <v-btn color="warning" variant="outlined" size="small" @click="resetPwd(item.name)">
            <v-icon start>mdi-key</v-icon>
            Reset password
          </v-btn>

          <v-btn color="error" variant="outlined" size="small" @click="deleteUser(item.name)">
            <v-icon start>mdi-delete</v-icon>
            Delete
          </v-btn>
        </v-col>
      </v-row>
    </v-card>

    <v-snackbar v-model="showChangeUserStatus" :color="changeUserStatus === 'Success' ? 'success' : 'error'"
      timeout="5000">
      {{ changeUserMsg }}
      <template v-slot:actions>
        <v-btn variant="text" @click="clearChangeUserStatus">Close</v-btn>
      </template>
    </v-snackbar>

    <v-card class="pa-4">
      <v-card-title class="text-h5 pb-2">Add User</v-card-title>
      <v-form @submit.prevent="addUser">
        <v-text-field v-model="name" label="Name" prepend-inner-icon="mdi-account" variant="outlined"
          class="mb-2"></v-text-field>

        <v-text-field v-model="pwd1" label="Password" prepend-inner-icon="mdi-lock" type="password" variant="outlined"
          class="mb-2"></v-text-field>

        <v-text-field v-model="pwd2" label="Confirm Password" prepend-inner-icon="mdi-lock-check" type="password"
          variant="outlined" class="mb-4"></v-text-field>

        <v-row>
          <v-col cols="12" sm="6">
            <v-checkbox v-model="is_admin" label="Is Admin"></v-checkbox>
          </v-col>
          <v-col cols="12" sm="6">
            <v-checkbox v-model="is_read_only" label="Is Read-only"></v-checkbox>
          </v-col>
        </v-row>

        <v-alert v-if="addUserStatus" :type="addUserStatus === 'Success' ? 'success' : 'error'" variant="tonal"
          class="my-4" closable @click:close="addUserStatus = ''">
          {{ addUserMsg }}
        </v-alert>

        <v-btn type="submit" color="primary" class="mt-2">
          <v-icon start>mdi-account-plus</v-icon>
          Add User
        </v-btn>
      </v-form>
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
  confirmMessage.value = `Are you sure you want to delete user "${name}"?`;
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
    ? `Make "${name}" read-only?`
    : `Remove read-only from "${name}"?`;

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
.gap-2 {
  gap: 8px;
}
</style>
