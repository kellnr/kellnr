<template>
  <v-app>
    <!-- Full-width Header -->
    <Header />

    <!-- Main content area with centered container -->
    <v-main>
      <div class="content-wrapper">
        <router-view></router-view>
      </div>
    </v-main>

    <!-- Full-width Footer -->
    <Footer />
  </v-app>
</template>

<script setup lang="ts">
import Footer from "./components/Footer.vue";
import Header from "./components/Header.vue";
</script>

<style>
html,
body {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
}

p {
  padding-bottom: 0.5em;
  padding-top: 0.5em;
}

/* Ensure v-app takes full height */
.v-application {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
}

/* Ensure main content grows to fill available space */
.v-main {
  flex: 1 0 auto;
  background-color: var(--v-background-base, #ffffff);
}

/* Centered content wrapper with responsive width */
.content-wrapper {
  width: 100%;
  margin: 0 auto;
  padding: 0 1rem;
  box-sizing: border-box;
}

/* Responsive breakpoints */
@media (min-width: 600px) {
  .content-wrapper {
    width: 95%;
    padding: 0 1.5rem;
  }
}

@media (min-width: 768px) {
  .content-wrapper {
    width: 90%;
    max-width: 1200px;
    padding: 0 2rem;
  }
}

@media (min-width: 992px) {
  .content-wrapper {
    width: 80%;
    /* Ensures at least 10% space on each side */
    padding: 0;
  }
}

@media (min-width: 1200px) {
  .content-wrapper {
    width: 75%;
  }
}

@media (min-width: 1600px) {
  .content-wrapper {
    width: 70%;
  }
}
</style>
