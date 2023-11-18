<template>
  <div id="header">
    <Header id="header-pane"/>
  </div>
  <main id="main">
    <section id="main-pane">
      <router-view></router-view>
    </section>
  </main>
  <Footer id="footer"/>
</template>

<script setup lang="ts">
import Footer from "./components/Footer.vue";
import Header from "./components/Header.vue";
</script>

<style>
body {
  margin: 0;
}

p {
  padding-bottom: 0.5em;
  padding-top: 0.5em;
}

#app {
  height: 100vh;
  display: grid;
  grid-template-columns: 1fr;
  grid-template-rows: auto 1fr auto;
}

#header {
  grid-row: 1;
  display: grid;
  grid-template-columns: 1fr 3fr 1fr;
}

#header > #header-pane {
  grid-column: 2;
}

#main {
  grid-row: 2;
  overflow: auto;
  display: grid;
  grid-template-rows: 1fr;
  grid-template-columns: 1fr 3fr 1fr;
}

#main > #main-pane {
  grid-column: 2;
}

#footer {
  grid-row: 3;
}
</style>

