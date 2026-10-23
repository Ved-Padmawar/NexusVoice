import type { ComponentProps, ReactElement, ReactNode } from "react"
import { Tooltip as TooltipPrimitive } from "radix-ui"

type TooltipProps = {
  content: ReactNode
  /** The element that triggers it; it receives the hover and focus handlers. */
  children: ReactElement
  side?: ComponentProps<typeof TooltipPrimitive.Content>["side"]
}

/** The app's own tooltip, in place of the browser's `title` bubble. */
function Tooltip({ content, children, side = "top" }: TooltipProps) {
  return (
    <TooltipPrimitive.Provider delayDuration={400}>
      <TooltipPrimitive.Root>
        <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
        <TooltipPrimitive.Portal>
          <TooltipPrimitive.Content
            side={side}
            sideOffset={6}
            collisionPadding={8}
            className="z-50 max-w-60 rounded-(--r-sm) border border-(--border) bg-(--panel) px-2 py-1 text-[11px] font-medium text-(--fg) shadow-(--shadow-md)"
          >
            {content}
          </TooltipPrimitive.Content>
        </TooltipPrimitive.Portal>
      </TooltipPrimitive.Root>
    </TooltipPrimitive.Provider>
  )
}

export { Tooltip }
