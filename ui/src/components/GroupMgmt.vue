<template>
  <h2 class="k-h2">Groups</h2>
  <template v-for="item in items" :key="item.name">
    <div class="groupMgmt glass">
      <span class="groupName">{{ item.name }}</span>
      <span class="tag is-warning is-light">
        <a @click="editGroup(item.name)">Edit</a>
      </span>
      <span class="tag is-danger is-light">
        <a @click="deleteGroup(item.name)">Delete</a>
      </span>
    </div>
  </template>

  <status-notification :status="changeGroupStatus" @update:clear="changeGroupStatus = $event">
    {{ changeGroupMsg }}
  </status-notification>

  <div v-if="!editingGroup">
    <h3 class="k-h3">Add Group</h3>
    <form>
      <div class="field">
        <div class="control is-expanded has-icons-left">
          <input class="input is-info" v-model="name" placeholder="Name" type="text" />
          <span class="icon is-small is-left">
            <i class="fas fa-user-group"></i>
          </span>
        </div>
      </div>

      <status-notification :status="addGroupStatus" @update:clear="addGroupStatus = $event">
        {{ addGroupMsg }}
      </status-notification>

      <div class="control">
        <button class="button is-info" @click.prevent="addGroup">Add</button>
      </div>
    </form>
  </div>
  <div v-else>
    <h3 class="k-h3">Edit Group {{ editingGroup }}</h3>
    <h2 class="k-h2">Group users</h2>
    <template v-for="user in groupUsers" :key="user.name">
      <div class="groupUser">
        <span class="userName">{{ user.name }}</span>
        <span class="tag is-danger is-light">
          <a @click="deleteGroupUser(user.name)">Remove</a>
        </span>
      </div>
    </template>
    <status-notification :status="deleteUserStatus" @update:clear="deleteUserStatus = $event">
      {{ deleteUserMsg }}
    </status-notification>
    <h3 class="k-h3">Add group user</h3>
    <form class="mb-3">
      <div class="field has-addons">
        <div class="control is-expanded">
          <div class="select is-fullwidth">
            <select v-model="groupUserName" class="input is-info">
              <option v-for="option in availableUsers" :value="option.name">
                {{ option.name }}
              </option>
            </select>
          </div>
        </div>
        <div class="control">
          <button type="submit" class="button is-info" @click.prevent="addGroupUser">
            Add
          </button>
        </div>
      </div>
      <status-notification :status="addUserStatus" @update:clear="addUserStatus = $event">
        {{ addUserMsg }}
      </status-notification>
    </form>

    <div class="control">
      <button class="button is-info" @click.prevent="cancelEditGroup">
        Cancel
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import StatusNotification from "./StatusNotification.vue";
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
const items = ref([]);
const users = ref([]);
const groupUsers = ref([]);
const groupUserName = ref("");
const name = ref("");
const editingGroup = ref("");

const availableUsers = computed(() => {
  return users.value.filter(
    (user) =>
      !groupUsers.value.some((groupUser) => groupUser.name === user.name),
  );
});

onBeforeMount(() => {
  getGroups();
  getUsers();
});

function editGroup(groupName) {
  editingGroup.value = groupName;
  getGroupUsers();
}

function cancelEditGroup(groupName) {
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
        // Update user list
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
    // @ts-ignore
    .get(LIST_GROUPS, { cache: false }) // disable caching to get updated token list (TS doesn't recognize cache option)
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
    // @ts-ignore
    .get(LIST_USERS, { cache: false }) // disable caching to get updated token list (TS doesn't recognize cache option)
    .then((res) => {
      if (res.status == 200) {
        users.value = res.data;
      }
    })
    .catch((error) => {
      console.log(error);
    });
}

function deleteGroup(name: string) {
  if (confirm('Delete group "' + name + '"?')) {
    axios
      .delete(DELETE_GROUP(name))
      .then((res) => {
        if (res.status == 200) {
          changeGroupStatus.value = "Success";
          changeGroupMsg.value = 'Group "' + name + '" deleted';
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
      });
  }
}
function addGroupUser() {
  axios
    .put(GROUP_USER(editingGroup.value, groupUserName.value))
    .then((res) => {
      if (res.status == 200) {
        addUserStatus.value = "Success";
        addUserMsg.value = "Group user successfully added.";
        // Update user list
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

function deleteGroupUser(name: string) {
  if (confirm('Delete group user "' + name + '"?')) {
    axios
      .delete(GROUP_USER(editingGroup.value, name))
      .then((res) => {
        if (res.status == 200) {
          deleteUserStatus.value = "Success";
          deleteUserMsg.value = "Group user successfully deleted.";
          // Update user list
          getGroupUsers();
        }
      })
      .catch((error) => {
        if (error.response) {
          deleteUserStatus.value = "Error";
          deleteUserMsg.value = "Group user could not be deleted.";

          if (error.response.status == 404) {
            // "Unauthorized. Login first."
            router.push("/login");
          } else if (error.response.status == 500) {
            deleteUserMsg.value = "Group user could not be deleted";
          } else {
            deleteUserMsg.value = "Unknown error";
          }
        }
      });
  }
}

function getGroupUsers() {
  axios
    // disable caching to get updated token list (TS doesn't recognize cache option)
    // @ts-ignore
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
.groupMgmt {
  border-radius: 2px;
  margin: 0.5rem 0 0.5rem 0;
  padding: 0.5rem;
  display: grid;
  grid-template-columns: 1fr max-content max-content;
}

.groupUser {
  border-radius: 2px;
  margin: 0.5rem 0 0.5rem 0;
  padding: 0.5rem;
  display: grid;
  grid-template-columns: 1fr max-content;
}

.groupName {
  font-weight: bolder;
}
</style>
