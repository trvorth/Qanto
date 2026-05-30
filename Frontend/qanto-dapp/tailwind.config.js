/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        obsidian: '#050505',
        'neon-cyan': '#06b6d4',
        'quantum-purple': '#8b5cf6',
      },
      fontFamily: {
        sans: ['Inter', 'sans-serif'],
        mono: ['JetBrains Mono', 'monospace'],
      },
      boxShadow: {
        'quantum-glow': '0 0 25px rgba(6, 182, 212, 0.15)',
        'purple-glow': '0 0 25px rgba(139, 92, 246, 0.15)',
      }
    },
  },
  plugins: [],
}
