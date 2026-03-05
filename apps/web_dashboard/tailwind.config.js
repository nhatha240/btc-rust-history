/** @type {import('tailwindcss').Config} */
module.exports = {
    content: ['./src/**/*.{js,ts,jsx,tsx,mdx}'],
    theme: {
        extend: {
            fontFamily: {
                sans: ['Inter', 'ui-sans-serif', 'system-ui'],
                mono: ['JetBrains Mono', 'ui-monospace'],
            },
            colors: {
                surface: {
                    50: '#f8fafc',
                    100: '#f1f5f9',
                    900: '#0f172a',
                    800: '#1e293b',
                    700: '#334155',
                },
                accent: {
                    400: '#818cf8',
                    500: '#6366f1',
                    600: '#4f46e5',
                },
            },
        },
    },
    plugins: [],
};
