import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "@/lib/utils"

/**
 * Chrome lives in `.btn*` (index.css), not here, so a button presses the same
 * way whether it is rendered through this component or as a bare element.
 */
const buttonVariants = cva("btn", {
  variants: {
    variant: {
      default: "btn-primary",
      primary: "btn-primary",
      quiet: "btn-quiet",
      outline: "btn-quiet",
      secondary: "btn-quiet",
      ghost: "btn-ghost",
      destructive: "btn-danger",
      link: "btn-ghost underline-offset-4 hover:underline",
    },
    size: {
      default: "",
      sm: "btn-sm",
      xs: "btn-sm",
      lg: "btn-lg",
      icon: "w-8 px-0",
      "icon-sm": "btn-sm w-7 px-0",
    },
  },
  defaultVariants: { variant: "default", size: "default" },
})

function Button({
  className,
  variant = "default",
  size = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
  }) {
  const Comp = asChild ? Slot.Root : "button"

  return (
    <Comp
      data-slot="button"
      className={cn(buttonVariants({ variant, size }), className)}
      {...props}
    />
  )
}

// eslint-disable-next-line react-refresh/only-export-components
export { Button, buttonVariants }
