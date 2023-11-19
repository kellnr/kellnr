<template>
  <h2 class="k-h2">Change Password</h2>
  <form>
    <div class="field">
      <div class="control is-expanded has-icons-left">
        <input
            class="input is-info"
            v-model="old_pwd"
            placeholder="Old password"
            type="password"
        />
        <span class="icon is-small is-left">
          <i class="fas fa-lock"></i>
        </span>
      </div>
    </div>
    <div class="field">
      <div class="control has-icons-left">
        <input
            class="input is-info"
            v-model="new_pwd1"
            placeholder="New password"
            type="password"
        />
        <span class="icon is-small is-left">
          <i class="fas fa-lock"></i>
        </span>
      </div>
    </div>
    <div class="field">
      <div class="control has-icons-left">
        <input
            class="input is-info"
            v-model="new_pwd2"
            placeholder="Confirm new password"
            type="password"
        />
        <span class="icon is-small is-left">
          <i class="fas fa-lock"></i>
        </span>
      </div>
    </div>

    <status-notification :status="pwdChangeStatus" @update:clear="pwdChangeStatus = $event">
      {{ pwdChangeMsg }}
    </status-notification>

    <div class="control">
      <button class="button is-info" @click.prevent="changePwd()">Apply</button>
    </div>
  </form>
</template>

<script setup lang="ts">
import StatusNotification from "../components/StatusNotification.vue";
import {ref} from "vue";
import axios from "axios";
import {CHANGE_PWD} from "../remote-routes";

const pwdChangeStatus = ref("")
const pwdChangeMsg = ref("")
const old_pwd = ref("")
const new_pwd1 = ref("")
const new_pwd2 = ref("")

function changePwd() {
  const postData = {
    old_pwd: old_pwd.value,
    new_pwd1: new_pwd1.value,
    new_pwd2: new_pwd2.value,
  };
  axios
    .post(CHANGE_PWD, postData)
    .then((res) => {
      if (res.status == 200) {
        pwdChangeMsg.value = "Password changed";
        pwdChangeStatus.value = "Success";
      }
    })
    .catch((error) => {
      if (error.response) {
        pwdChangeStatus.value = "Error";
        if (error.response.status == 400) {
          pwdChangeMsg.value = "Password wrong or passwords do not match";
        } else if (error.response.status == 404) {
          pwdChangeMsg.value = "Unauthorized. Please login first.";
        } else if (error.response.status == 500) {
          pwdChangeMsg.value = "Internal server error";
        } else {
          pwdChangeMsg.value = "Unkown error";
        }
      }
    });
}
</script>

<style>
</style>
