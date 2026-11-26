type ViewportAnchor = { readonly line: number; dispose: () => void }

type FollowHooks = {
  canFollow: (restoring: boolean) => boolean
  atBottom: () => boolean
  scrollToBottom: () => void
  syncScrollbar?: () => void
  scheduleFrame: (callback: () => void) => number
  cancelFrame: (id: number) => void
  captureAnchor?: () => ViewportAnchor | undefined
  scrollToLine?: (line: number) => void
}

// User scroll intent survives TUI repaints, hidden panes and buffer switches.
// A programmatic viewport change alone does not mean the user is reading history.
export function createTermFollow(hooks: FollowHooks) {
  let following = true
  let frame: number | undefined
  let disposed = false
  let anchor: ViewportAnchor | undefined
  let restoring = false
  let suspended = false

  function remember() {
    anchor?.dispose()
    anchor = following ? undefined : hooks.captureAnchor?.()
  }

  function refresh() {
    if (disposed || frame !== undefined) return
    frame = hooks.scheduleFrame(() => {
      frame = undefined
      if (suspended || !hooks.canFollow(restoring)) return
      if (following) hooks.scrollToBottom()
      else if (restoring && anchor && anchor.line >= 0) hooks.scrollToLine?.(anchor.line)
      if (restoring) hooks.syncScrollbar?.()
      restoring = false
    })
  }

  return {
    isFollowing: () => following,
    isSuspended: () => suspended,
    refresh,
    pause() {
      if (suspended || disposed) return
      following = false; restoring = false; remember()
    },
    userScroll() {
      if (suspended || disposed) return
      following = hooks.atBottom()
      restoring = false
      remember()
      if (following) refresh()
    },
    suspend() {
      if (disposed || suspended) return
      suspended = true
      // Keep intent and a buffer marker, rather than sampling a viewport that
      // the renderer may already have moved during deactivation.
      if (!following && !anchor) remember()
      if (frame !== undefined) hooks.cancelFrame(frame)
      frame = undefined
    },
    resume() { suspended = false; restoring = true; refresh() },
    dispose() {
      disposed = true
      if (frame !== undefined) hooks.cancelFrame(frame)
      frame = undefined
      anchor?.dispose()
      anchor = undefined
    }
  }
}
