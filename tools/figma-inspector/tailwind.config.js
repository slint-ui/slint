import figmaKitPreset from "figma-kit/tailwind.preset.js";
/** @type {import('tailwindcss').Config} */
export default {
    presets: [figmaKitPreset],
    content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
    theme: {
        extend: {},
    },
    plugins: [],
};

