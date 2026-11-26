<script lang="ts">
  import { onMount } from "svelte";
  import {
    ChevronDown,
    Pause,
    Play,
    Repeat,
    Repeat1,
    Shuffle,
    SkipBack,
    SkipForward,
    Volume1,
    Volume2,
    VolumeX,
  } from "@lucide/svelte";
  import { api, formatTime, type Repeat as RepeatMode } from "./api";
  import { coverFor } from "./covers";
  import { app } from "./store.svelte";
  import Cover from "./Cover.svelte";
  import IconButton from "./IconButton.svelte";

  const nextRepeat: Record<RepeatMode, RepeatMode> = { off: "all", all: "one", one: "off" };
  const popEasing = "cubic-bezier(0.34, 1.45, 0.64, 1)";
  const smoothEasing = "cubic-bezier(0.65, 0, 0.35, 1)";
  const dismissDistance = 0.18;
  const dismissVelocity = 0.5;

  let player = $derived(app.player);
  let dragging: number | null = $state(null);
  let backdrop: string | null = $state(null);

  let position = $derived(dragging ?? player.position);
  let progress = $derived(player.duration > 0 ? (position / player.duration) * 100 : 0);
  /** On Android the phone's own volume buttons control loudness. */
  const showVolume = !("MossampAndroid" in window);
  let volumeIcon = $derived(player.volume === 0 ? VolumeX : player.volume < 0.5 ? Volume1 : Volume2);

  let blur: HTMLDivElement;
  let shell: HTMLDivElement;
  let panel: HTMLDivElement;
  let closing = false;

  let swipeStart: number | null = null;
  let swipeOffset = 0;
  let lastY = 0;
  let lastTime = 0;
  let velocity = 0;

  $effect(() => {
    const path = player.track?.path;
    backdrop = null;
    if (path) coverFor(path).then((cover) => (backdrop = cover));
  });

  function settle(animation: Animation): Promise<void> {
    return animation.finished
      .then(() => {
        animation.commitStyles();
        animation.cancel();
      })
      .catch(() => {});
  }

  function freeze(element: HTMLElement) {
    for (const animation of element.getAnimations()) {
      animation.commitStyles();
      animation.cancel();
    }
  }

  function coverScale(): number {
    return (Math.hypot(innerWidth, innerHeight) / shell.offsetWidth) * 1.05;
  }

  onMount(() => {
    const scale = coverScale();

    settle(blur.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 280, easing: "ease-out", fill: "forwards" }));
    settle(
      shell.animate(
        [
          { transform: "scale(0.15)", opacity: 0, easing: popEasing },
          { transform: "scale(1)", opacity: 1, offset: 0.42, easing: smoothEasing },
          { transform: `scale(${scale})`, opacity: 1 },
        ],
        { duration: 760, fill: "forwards" },
      ),
    );
    settle(
      panel.animate(
        [
          { opacity: 0, transform: "translateY(24px)" },
          { opacity: 1, transform: "translateY(0)" },
        ],
        { duration: 300, delay: 600, easing: "ease-out", fill: "both" },
      ),
    );
  });

  async function close() {
    if (closing) return;
    closing = true;
    [blur, shell, panel].forEach(freeze);
    const scale = coverScale();

    await Promise.all([
      settle(
        panel.animate([{ opacity: 1 }, { opacity: 0, transform: "scale(0.97)" }], {
          duration: 180,
          easing: "ease-in",
          fill: "forwards",
        }),
      ),
      settle(
        shell.animate(
          [
            { transform: `scale(${scale})`, opacity: 1, easing: smoothEasing },
            { transform: "scale(1)", opacity: 1, offset: 0.62, easing: "ease-in" },
            { transform: "scale(0.15)", opacity: 0 },
          ],
          { duration: 600, fill: "forwards" },
        ),
      ),
      settle(blur.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 300, delay: 320, fill: "forwards" })),
    ]);
    app.fullPlayerOpen = false;
  }

  function startSwipe(event: PointerEvent) {
    if (closing || (event.target as HTMLElement).closest("input, button")) return;
    [blur, shell, panel].forEach(freeze);
    shell.style.opacity = "0";
    panel.classList.add("lifted");
    panel.setPointerCapture(event.pointerId);

    swipeStart = event.clientY;
    swipeOffset = 0;
    lastY = event.clientY;
    lastTime = event.timeStamp;
    velocity = 0;
  }

  function moveSwipe(event: PointerEvent) {
    if (swipeStart === null) return;
    const elapsed = event.timeStamp - lastTime;
    if (elapsed > 0) velocity = (event.clientY - lastY) / elapsed;
    lastY = event.clientY;
    lastTime = event.timeStamp;

    swipeOffset = Math.max(0, event.clientY - swipeStart);
    panel.style.transform = `translateY(${swipeOffset}px)`;
    blur.style.opacity = String(1 - Math.min(swipeOffset / innerHeight, 1));
  }

  function endSwipe() {
    if (swipeStart === null) return;
    swipeStart = null;

    if (swipeOffset > innerHeight * dismissDistance || velocity > dismissVelocity) {
      dismiss();
    } else {
      snapBack();
    }
  }

  async function dismiss() {
    closing = true;
    const remaining = innerHeight - swipeOffset;
    const duration = Math.min(340, Math.max(180, remaining / Math.max(velocity, 1.6)));

    await Promise.all([
      settle(
        panel.animate(
          [{ transform: `translateY(${swipeOffset}px)` }, { transform: `translateY(${innerHeight}px)` }],
          { duration, easing: "cubic-bezier(0.2, 0.7, 0.4, 1)", fill: "forwards" },
        ),
      ),
      settle(
        blur.animate([{ opacity: blur.style.opacity || "1" }, { opacity: 0 }], {
          duration: duration + 60,
          easing: "ease-out",
          fill: "forwards",
        }),
      ),
    ]);
    app.fullPlayerOpen = false;
  }

  function snapBack() {
    const easing = "cubic-bezier(0.34, 1.25, 0.64, 1)";
    settle(
      panel.animate([{ transform: `translateY(${swipeOffset}px)` }, { transform: "translateY(0)" }], {
        duration: 420,
        easing,
        fill: "forwards",
      }),
    ).then(() => panel.classList.remove("lifted"));
    settle(blur.animate([{ opacity: blur.style.opacity || "1" }, { opacity: 1 }], { duration: 300, fill: "forwards" }));
  }

  function commitSeek(value: number) {
    api.seek(value);
    dragging = null;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="root">
  <div class="blur" bind:this={blur} style="opacity: 0"></div>
  <div class="shell" bind:this={shell} style="opacity: 0"></div>

  <div
    class="panel"
    role="dialog"
    aria-modal="true"
    aria-label="Now playing"
    tabindex="-1"
    bind:this={panel}
    style="opacity: 0"
    onpointerdown={startSwipe}
    onpointermove={moveSwipe}
    onpointerup={endSwipe}
    onpointercancel={endSwipe}
  >
    {#if backdrop}
      <img class="backdrop" src={backdrop} alt="" decoding="async" />
    {/if}
    <div class="shade"></div>

    {#if player.track}
      <div class="content">
        <div class="grabber"></div>
        <header>
          <IconButton icon={ChevronDown} label="Close player" variant="soft" onclick={close} />
          <span class="now" class:live={player.playing}>Now playing</span>
          <span class="spacer"></span>
        </header>

        <div class="art">
          <Cover path={player.track.path} rounded="large" />
        </div>

        <div class="details">
          <h1>{player.track.title}</h1>
          <p>{player.track.artist} · {player.track.album}</p>
        </div>

        <div class="seek">
          <input
            type="range"
            min="0"
            max={player.duration || 1}
            step="0.5"
            value={position}
            style="--progress: {progress}%"
            oninput={(e) => (dragging = Number(e.currentTarget.value))}
            onchange={(e) => commitSeek(Number(e.currentTarget.value))}
          />
          <div class="times">
            <span>{formatTime(position)}</span>
            <span>{formatTime(player.duration)}</span>
          </div>
        </div>

        <div class="controls">
          <IconButton
            icon={Shuffle}
            label="Shuffle"
            active={player.shuffle}
            onclick={() => api.setShuffle(!player.shuffle)}
          />
          <IconButton icon={SkipBack} label="Previous" variant="soft" size="lg" solid onclick={() => api.previous()} />
          <IconButton
            icon={player.playing ? Pause : Play}
            label={player.playing ? "Pause" : "Play"}
            variant="filled"
            size="xl"
            solid
            onclick={() => api.toggle()}
          />
          <IconButton icon={SkipForward} label="Next" variant="soft" size="lg" solid onclick={() => api.next()} />
          <IconButton
            icon={player.repeat === "one" ? Repeat1 : Repeat}
            label="Repeat: {player.repeat}"
            active={player.repeat !== "off"}
            onclick={() => api.setRepeat(nextRepeat[player.repeat])}
          />
        </div>

        {#if showVolume}
          <div class="volume">
            <IconButton
              icon={volumeIcon}
              label="Mute"
              size="sm"
              onclick={() => api.setVolume(player.volume > 0 ? 0 : 0.8)}
            />
            <input
              type="range"
              min="0"
              max="1"
              step="0.01"
              value={player.volume}
              style="--progress: {player.volume * 100}%"
              oninput={(e) => api.setVolume(Number(e.currentTarget.value))}
            />
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .root {
    position: fixed;
    inset: 0;
    z-index: 50;
    overflow: hidden;
  }

  .blur {
    position: absolute;
    inset: 0;
    background: rgba(10, 11, 10, 0.45);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
    will-change: opacity;
  }

  .shell {
    position: absolute;
    left: calc(50% - 21vmin);
    top: calc(50% - 21vmin);
    width: 42vmin;
    height: 42vmin;
    border-radius: 36px;
    background: var(--bg-soft);
    box-shadow: var(--shadow);
    will-change: transform, opacity;
  }

  .panel {
    position: absolute;
    inset: 0;
    overflow: hidden;
    background: var(--bg-soft);
    touch-action: none;
    will-change: transform, opacity;
    transition: border-radius 0.25s;
  }

  .panel:global(.lifted) {
    border-radius: 32px 32px 0 0;
  }

  .backdrop {
    position: absolute;
    inset: -10%;
    width: 120%;
    height: 120%;
    object-fit: cover;
    filter: blur(40px) saturate(1.2);
    opacity: 0.5;
    transform: translateZ(0);
  }

  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      to bottom,
      color-mix(in srgb, var(--bg) 55%, transparent),
      color-mix(in srgb, var(--bg) 92%, transparent) 70%
    );
  }

  .content {
    position: relative;
    height: 100%;
    max-width: 30rem;
    margin: 0 auto;
    padding: 0.5rem 1.4rem 1.8rem;
    display: flex;
    flex-direction: column;
    gap: 1.2rem;
  }

  .grabber {
    flex-shrink: 0;
    align-self: center;
    width: 2.6rem;
    height: 5px;
    border-radius: var(--pill);
    background: var(--fg-muted);
    opacity: 0.4;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    color: var(--fg-muted);
    font-size: 0.8rem;
    font-weight: 800;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  .spacer {
    width: 2.9rem;
  }

  /* While playing, a soft highlight sweeps through the text from left to
     right. The text keeps its usual muted colour; only the light band moves. */
  .now.live {
    background: linear-gradient(
        90deg,
        var(--fg-muted) 0%,
        var(--fg-muted) 40%,
        color-mix(in srgb, var(--fg-muted) 40%, var(--fg)) 50%,
        var(--fg-muted) 60%,
        var(--fg-muted) 100%
      )
      0 0 / 250% 100% no-repeat;
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    animation: now-sweep 3.5s ease-in-out infinite;
  }

  @keyframes now-sweep {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: 0% 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .now.live {
      animation: none;
      background: none;
      color: inherit;
    }
  }

  .art {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    container-type: size;
  }

  .art :global(.cover) {
    width: min(100cqw, 100cqh) !important;
  }

  .details {
    text-align: center;
  }

  h1 {
    font-size: 1.6rem;
    font-weight: 900;
    line-height: 1.2;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .details p {
    margin-top: 0.3rem;
    color: var(--fg-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .seek input {
    width: 100%;
  }

  .times {
    display: flex;
    justify-content: space-between;
    margin-top: 0.4rem;
    color: var(--fg-muted);
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
  }

  .controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .volume {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .volume input {
    flex: 1;
  }
</style>
