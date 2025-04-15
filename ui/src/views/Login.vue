<template>
  <v-container fluid class="fill-height">
    <v-row align="center" justify="center">
      <v-col cols="12" sm="8" md="6" lg="4">
        <v-card class="elevation-12">
          <v-card-title class="text-center text-h5">
            Sign In
          </v-card-title>
          <v-card-text>
            <v-form ref="form" @submit.prevent="submit" v-model="isFormValid">
              <v-text-field v-model="user" label="User" prepend-inner-icon="fas fa-user" variant="outlined"
                :rules="userRules" required></v-text-field>

              <v-text-field v-model="pwd" label="Password" prepend-inner-icon="fas fa-lock" type="password"
                variant="outlined" :rules="passwordRules" required></v-text-field>

              <v-checkbox v-model="store.rememberMe" label="Remember me" class="mt-2 checkbox-fix"></v-checkbox>

              <v-alert v-if="loginStatus" :type="loginStatus === 'Success' ? 'success' : 'error'" class="mt-2"
                density="compact" closable @click:close="loginStatus = ''">
                {{ loginStatusMsg }}
              </v-alert>

              <div class="text-center mt-4">
                <v-btn color="primary" size="large" type="submit" block :disabled="!isFormValid">
                  Confirm
                </v-btn>
              </div>
            </v-form>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import axios from "axios";
import { useStore } from "../store/store";
import { LOGIN } from "../remote-routes";
import router from "../router";

const form = ref(null);
const isFormValid = ref(false);
const loginStatusMsg = ref("");
const loginStatus = ref(""); // "", "Error", "Success"
const user = ref("");
const pwd = ref("");
const store = useStore();

// Validation rules
const userRules = [
  (v: string) => !!v || 'Username is required',
];

const passwordRules = [
  (v: string) => !!v || 'Password is required',
];

onMounted(() => {
  if (store.rememberMe && store.rememberMeUser !== null) {
    user.value = store.rememberMeUser;
  }
});

function submit() {
  if (!isFormValid.value) {
    return; // Don't submit if form is not valid
  }

  const postData = { user: user.value, pwd: pwd.value };
  axios
    .post(LOGIN, postData)
    .then((res) => {
      if (res.status == 200) {
        loginStatusMsg.value = "Login successful";
        loginStatus.value = "Success";
        store.login(res.data);
        if (store.rememberMe) {
          store.rememberMeUser = user.value;
        }
        if (router.currentRoute.value.query["redirect"] === "settings") {
          router.push("/settings");
        } else {
          router.push("/");
        }
      }
    })
    .catch((error) => {
      if (error.response) {
        loginStatus.value = "Error";
        if (error.response.status == 401) {
          loginStatusMsg.value = "Wrong user or password";
        } else if (error.response.status == 500) {
          loginStatusMsg.value = "Internal server error";
        } else {
          loginStatusMsg.value = "Unknown error";
        }
      }
    });
}
</script>
