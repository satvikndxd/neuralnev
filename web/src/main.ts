import "./styles.css";
import { renderLatencyChart } from "./components/chart";
import { renderMeters } from "./components/meters";
import { mountConsole } from "./views/console";

renderLatencyChart();
renderMeters();
mountConsole();
