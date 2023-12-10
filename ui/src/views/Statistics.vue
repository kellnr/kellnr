<template>
  <div id="statistics">
    <h1>Statistics</h1>
    <div class="statisticsCards">
      <statistics-card2 :num=10 :icon="icon" :text="text"></statistics-card2>
      <statistics-card2 :num=10 :icon="icon" :text="text"></statistics-card2>
      <statistics-card2 :num=10 :icon="icon" :text="text"></statistics-card2>
      <statistics-card2 :num=10 :icon="icon" :text="text"></statistics-card2>
      <statistics-card2 :num=10 :icon="icon" :text="text"></statistics-card2>
      <statistics-card2 :num=10 :icon="icon" :text="text"></statistics-card2>
      <statistics-card2 :num=10 :icon="icon" :text="text"></statistics-card2>
      <statistics-card2 :num=10 :icon="icon" :text="text"></statistics-card2>
      <statistics-card2 :num=10 :icon="icon" :text="text"></statistics-card2>
      <statistics-card2 :num=10 :icon="icon" :text="text"></statistics-card2>
      <statistics-card2 :num=10 :icon="icon" :text="text"></statistics-card2>
      <statistics-card2 :num=10 :icon="icon" :text="text"></statistics-card2>
      <statistics-card2 :num=10 :icon="icon" :text="text"></statistics-card2>
      <statistics-card2 :num=10 :icon="icon" :text="text"></statistics-card2>
    </div>
  </div>
</template>

<script setup lang="ts">
import axios from 'axios';
import StatisticsCard2 from '../components/StatisticsCard2.vue';

const icon = 'fa-boxes';
const text = 'number of downloads';
</script>

<style scoped>
.statisticsCards {
  display: flex;
  flex-wrap: wrap;
}
</style>
