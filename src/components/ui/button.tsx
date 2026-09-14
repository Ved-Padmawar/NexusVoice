import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "@/lib/utils"

const buttonVariants = cva("nv-btn", {
  variants: {
    variant: {
      primary: "nv-btn--primary",
      secondary: "nv-btn--secondary",
      ghost: "nv-btn--ghost",
      danger: "nv-btn--danger",
    },
    size: {
      sm: "nv-btn--sm",
      default: "",
    },
  },
  defaultVariants: {
    variant: "primary",
    size: "default",
  },
})

function Button({
  className,
  variant,
  size,
  asChild = false,
  type = "button",
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
  }) {
  const Comp = asChild ? Slot.Root : "button"

  return (
    <Comp
      data-slot="button"
      type={asChild ? undefined : type}
      className={cn(buttonVariants({ variant, size }), className)}
      {...props}
    />
  )
}

type IconButtonProps = React.ComponentProps<"button"> & {
  /** Accessible name; also the tooltip. */
  label: string
  tone?: "default" | "accent" | "danger" | "success"
  size?: "sm" | "default"
  framed?: boolean
}

function IconButton({ label, tone = "default", size = "default", framed, className, type = "button", ...props }: IconButtonProps) {
  return (
    <button
      type={type}
      aria-label={label}
      title={label}
      className={cn(
        "nv-icon-btn",
        tone !== "default" && `nv-icon-btn--${tone}`,
        size === "sm" && "nv-icon-btn--sm",
        framed && "nv-icon-btn--framed",
        className,
      )}
      {...props}
    />
  )
}

export { Button, IconButton }
