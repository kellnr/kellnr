<template>
  <Header id="header"/>
  <main id="main">
      <router-view></router-view>
  </main>
  <Footer id="footer"/>
</template>

<script setup lang="ts">
import Footer from "./components/Footer.vue";
import Header from "./components/Header.vue";
</script>

<style>
html,
body {
  margin: 0;
  width: 100%;
  height: 100%;
}

p {
  padding-bottom: 0.5em;
  padding-top: 0.5em;
}

#app {
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
  height: 100%;
  width: 100%;
}

#header {
  margin: 1rem 0;
}

#main {
  height: 100%;
  overflow: auto;
}

@media only screen and (max-width: 768px) {
  #main {
    padding: 0 1rem;
  }
}

@media only screen and (min-width: 768px) {
  #main {
    padding: 0 15%;
  }
}

@media only screen and (min-width: 992px) {
  #main {
    padding: 0 20%;
  }
}

</style>
