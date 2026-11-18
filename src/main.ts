import { mount } from "svelte";
import "@fontsource-variable/nunito";
import "./theme.css";
import App from "./App.svelte";

mount(App, { target: document.getElementById("app")! });
