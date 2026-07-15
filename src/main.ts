import { createApp } from "vue";
import "./style.css";
import { installDemo } from "./demo";
import App from "./App.vue";

installDemo();
createApp(App).mount("#app");
