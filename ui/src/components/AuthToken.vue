<template>
  <h2 class="k-h2">Authentication Tokens</h2>
    <template v-for="item in items" :key="item.name">
      <div class="authToken glass">
        <span class="tokenName">{{ item.name }}</span>
        <span class="tag is-danger is-light">
          <a @click="deleteToken(item.name, item.id)">Delete</a>
        </span>
      </div>
    </template>
  <form>
    <div class="field">
      <div class="control is-expanded has-icons-left">
        <input
            class="input is-info"
            v-model="name"
            placeholder="Descriptive name for the token"
            type="text"
        />
        <span class="icon is-small is-left">
          <i class="fas fa-align-center"></i>
        </span>
      </div>
    </div>

    <status-notification :status="addTokenStatus" @update:clear="addTokenStatus = $event">
      {{ addTokenMsg }}
    </status-notification>

    <div class="control">
      <button class="button is-info" @click.prevent="addToken()">Add</button>
    </div>
  </form>
</template>

<script setup lang="ts">
import {onBeforeMount, ref} from 'vue'
import StatusNotification from "../components/StatusNotification.vue";
import axios from "axios";
import {useRouter} from "vue-router";
import {ADD_TOKEN, DELETE_TOKEN, kellnr_url, LIST_TOKENS} from "../remote-routes";

const addTokenStatus = ref("")
const addTokenMsg = ref("")
const items = ref([])
const name = ref("")
const router = useRouter()

onBeforeMount(() => {
  getTokens()
})

function addToken() {
  const postData = {
    name: name.value,
  };

  axios
      .post(ADD_TOKEN, postData)
      .then((res) => {
        if (res.status == 200) {
          addTokenMsg.value =
              'New authentication token: "' +
              res.data["token"] +
              '". Copy and save the token as it cannot be displayed again. Do not share the token.';
          addTokenStatus.value = "Success";
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
      });
}

function getTokens() {
  axios
      .get(LIST_TOKENS, { cache: false }) // disable caching to get updated token list (TS doesn't recognize cache option)
      .then((res) => {
        if (res.status == 200) {
          items.value = res.data;
        }
      })
      .catch((error) => {
        console.log(error);
      });
}

function deleteToken(name: String, id: number) {
  if (confirm('Delete token "' + name + '"?')) {
    axios
        .delete(DELETE_TOKEN(id))
        .then(() => {
          // Update shown token list
          getTokens();
        })
        .catch((error) => console.log(error));
  }
  getTokens()
}
</script>

<style scoped>
.authToken {
  border-radius: 2px;
  margin: 0.5rem 0 0.5rem 0;
  padding: 0.5rem;
  display: grid;
  grid-template-columns: 1fr max-content;
}

.tokenName {
  font-weight: bolder;
}

</style>
