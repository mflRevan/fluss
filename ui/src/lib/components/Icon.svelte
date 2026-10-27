<script lang="ts">
  // Small stroke icon set (24px grid, 1.75 stroke).
  let { name, size = 18, stroke = 1.75 }: { name: string; size?: number; stroke?: number } = $props();

  const paths: Record<string, string> = {
    home: "M3 10.5 12 3l9 7.5M5 9v11h5v-6h4v6h5V9",
    book: "M4 19.5V5a2 2 0 0 1 2-2h13v15H6.5A2.5 2.5 0 0 0 4 20.5 2.5 2.5 0 0 0 6.5 23H19M8 7h7M8 11h5",
    keyboard: "M3 6h18a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1ZM6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10",
    cpu: "M6 6h12v12H6zM9 9h6v6H9zM9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4",
    mic: "M12 3a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V6a3 3 0 0 0-3-3ZM5 11a7 7 0 0 0 14 0M12 18v3M8 21h8",
    type: "M4 7V5h16v2M9 19h6M12 5v14",
    output: "M4 12h12M12 6l6 6-6 6M20 4v16",
    sliders: "M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0M14 4v4M8 10v4M16 16v4",
    info: "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20ZM12 16v-5M12 8h.01",
    search: "M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16ZM21 21l-4.35-4.35",
    plus: "M12 5v14M5 12h14",
    trash: "M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14",
    x: "M18 6 6 18M6 6l12 12",
    check: "M20 6 9 17l-5-5",
    chevron: "m9 18 6-6-6-6",
    chevronDown: "m6 9 6 6 6-6",
    lock: "M6 11h12v10H6zM8 11V7a4 4 0 0 1 8 0v4",
    sparkles: "M12 3l1.8 4.6L18 9.4l-4.2 1.8L12 16l-1.8-4.8L6 9.4l4.2-1.8ZM19 15l.7 1.8 1.8.7-1.8.7L19 20l-.7-1.8-1.8-.7 1.8-.7ZM5 15l.6 1.4L7 17l-1.4.6L5 19l-.6-1.4L3 17l1.4-.6Z",
    download: "M12 3v12M7 10l5 5 5-5M5 21h14",
    minimize: "M5 12h14",
    maximize: "M5 5h14v14H5z",
    close: "M6 6l12 12M18 6 6 18",
    edit: "M12 20h9M16.5 3.5a2.1 2.1 0 1 1 3 3L7 19l-4 1 1-4Z",
    arrowRight: "M5 12h14M13 6l6 6-6 6",
    arrowLeft: "M19 12H5M11 18l-6-6 6-6",
    folder: "M3 6a1 1 0 0 1 1-1h5l2 2h9a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1Z",
    file: "M14 3H6v18h12V7ZM14 3v4h4",
    power: "M12 3v9M6.3 6.3a8 8 0 1 0 11.4 0",
    refresh: "M3 12a9 9 0 0 1 15.5-6.3L21 8M21 3v5h-5M21 12a9 9 0 0 1-15.5 6.3L3 16M3 21v-5h5",
    zap: "M13 2 4 14h7l-1 8 9-12h-7Z",
    globe: "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20ZM2 12h20M12 2a15 15 0 0 1 0 20M12 2a15 15 0 0 0 0 20",
    clock: "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20ZM12 6v6l4 2",
    wave: "M2 12c2.5-4 5-4 7.5 0s5 4 7.5 0 3.5-4 5 0",
    gpu: "M2 7h18v10H2zM20 10h2M20 14h2M6 17v3M10 17v3M14 17v3M11 12a2 2 0 1 0 0 .01",
    memory: "M4 7h16v10H4zM8 7v10M12 7v10M16 7v10M4 4v3M20 4v3M4 17v3M20 17v3",
    play: "M7 4v16l13-8Z",
    stop: "M6 6h12v12H6z",
    hand: "M8 13V5.5a1.5 1.5 0 0 1 3 0V12M11 11V4.5a1.5 1.5 0 0 1 3 0V12M14 11.5V6a1.5 1.5 0 0 1 3 0v8a7 7 0 0 1-7 7h-.5A6.5 6.5 0 0 1 4 17.5l-1.2-3.2a1.6 1.6 0 0 1 2.9-1.3L8 16",
    quote: "M7 7h4v4c0 3-2 5-4 5M15 7h4v4c0 3-2 5-4 5",
    shield: "M12 2 4 5v6c0 5 3.5 9.5 8 11 4.5-1.5 8-6 8-11V5Z",
    link: "M10 13a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1M14 11a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1",
    eye: "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12ZM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
    eyeOff: "M3 3l18 18M10.6 5.1A10.8 10.8 0 0 1 12 5c6.5 0 10 7 10 7a17 17 0 0 1-3.2 4.1M6.6 6.6A17 17 0 0 0 2 12s3.5 7 10 7a10 10 0 0 0 5.4-1.6M9.9 9.9a3 3 0 0 0 4.2 4.2",
    undo: "M9 14 4 9l5-5M4 9h11a5 5 0 0 1 0 10h-3",
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={paths[name] ?? ""} />
</svg>
