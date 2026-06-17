<template>
  <router-view />
</template>

<style>
/* 全局暗色主题 */
:root {
  --el-color-primary: #58a6ff;
  --el-color-success: #3fb950;
  --el-color-warning: #f0883e;
  --el-color-danger: #f85149;
  --el-bg-color: #0d1117;
  --el-bg-color-overlay: #161b22;
  --el-text-color-primary: #e6edf3;
  --el-text-color-regular: #c9d1d9;
  --el-text-color-secondary: #8b949e;
  --el-border-color: #30363d;
  --el-fill-color-blank: #0d1117;
}

body {
  margin: 0;
  padding: 0;
  font-family: -apple-system, "PingFang SC", "Microsoft YaHei", sans-serif;
  background: #0d1117;
  color: #c9d1d9;
  height: 100vh;
  overflow: hidden;
}

#app {
  height: 100vh;
}
</style>
