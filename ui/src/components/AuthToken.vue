<template>
  <v-container>
    <h2 class="text-h4 mb-4">Authentication Tokens</h2>

    <v-list v-if="items.length > 0" class="mb-4">
      <v-list-item v-for="item in items" :key="item.name" class="mb-2">
        <v-card width="100%" elevation="1" class="pa-3">
          <div class="d-flex justify-space-between align-center">
            <span class="font-weight-bold">{{ item.name }}</span>
            <v-btn size="small" color="error" variant="outlined" @click="deleteToken(item.name, item.id)">
              Delete
            </v-btn>
          </div>
        </v-card>
      </v-list-item>
    </v-list>

    <v-card class="pa-4 mb-4">
      <v-form @submit.prevent="addToken">
        <v-text-field v-model="name" label="Descriptive name for the token" prepend-inner-icon="mdi-tag"
          variant="outlined" class="mb-4"></v-text-field>

        <v-alert v-if="addTokenStatus" :type="addTokenStatus === 'Success' ? 'success' : 'error'" closable
          variant="tonal" @update:model-value="addTokenStatus = ''" class="mb-4">
          <div class="d-flex flex-column">
            <p class="mb-2">{{ addTokenMsg }}</p>

            <v-card v-if="addedToken" variant="outlined" class="pa-3 mb-2">
              <div class="d-flex justify-space-between align-center">
                <code class="token-text">{{ addedToken }}</code>
                <v-btn size="large" color="primary" variant="elevated" @click="copyToken" class="ml-4">
                  <v-icon class="mr-2">mdi-content-copy</v-icon>
                  Copy
                </v-btn>
              </div>
            </v-card>
          </div>
        </v-alert>

        <v-btn color="primary" type="submit" :loading="loading">
          Add
        </v-btn>
      </v-form>
    </v-card>
  </v-container>
</template>

<script setup lang="ts">
import { onBeforeMount, ref } from "vue";
import axios from "axios";
import { useRouter } from "vue-router";
import { ADD_TOKEN, DELETE_TOKEN, LIST_TOKENS } from "../remote-routes";

const addTokenStatus = ref("");
const addTokenMsg = ref("");
const addedToken = ref("");
const items = ref([]);
const name = ref("");
const loading = ref(false);
const router = useRouter();

onBeforeMount(() => {
  getTokens();
});

function copyToken() {
  navigator.clipboard.writeText(addedToken.value)
    .then(() => {
      // Optionally provide feedback that the token was copied
      const originalMessage = addTokenMsg.value;
      addTokenMsg.value = "Token copied to clipboard!";
      setTimeout(() => {
        addTokenMsg.value = originalMessage;
      }, 2000);
    })
    .catch(err => {
      console.error('Failed to copy token: ', err);
    });
}

function addToken() {
  if (!name.value.trim()) {
    addTokenStatus.value = "Error";
    addTokenMsg.value = "Please enter a name for the token";
    return;
  }

  loading.value = true;
  const postData = {
    name: name.value,
  };

  axios
    .post(ADD_TOKEN, postData)
    .then((res) => {
      if (res.status == 200) {
        addedToken.value = res.data["token"];
        addTokenMsg.value =
          "New authentication token added. Copy and save the token as it cannot be displayed again. Do not share the token.";
        addTokenStatus.value = "Success";
        name.value = ""; // Clear the input field
        // update shown token list
        getTokens();
      }
    })
    .catch((error) => {
      if (error.response) {
        addTokenStatus.value = "Error";
        if (error.response.status == 404) {
          // "Unauthorized. Login first."
          router.push("/login");
        } else if (error.response.status == 500) {
          addTokenMsg.value = "Token could not be created";
        } else {
          addTokenMsg.value = "Unknown error";
        }
      }
    })
    .finally(() => {
      loading.value = false;
    });
}

function getTokens() {
  axios
    .get(LIST_TOKENS, { cache: false })
    .then((res) => {
      if (res.status == 200) {
        items.value = res.data;
      }
    })
    .catch((error) => {
      console.log(error);
    });
}

function deleteToken(name: string, id: number) {
  if (confirm('Delete token "' + name + '"?')) {
    axios
      .delete(DELETE_TOKEN(id))
      .then(() => {
        // Update shown token list
        getTokens();
      })
      .catch((error) => console.log(error));
  }
}
</script>

<style scoped>
.token-text {
  word-break: break-all;
  font-family: monospace;
  font-size: 1rem;
  flex: 1;
}
</style>
