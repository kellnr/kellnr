export type UserRole = "user" | "read-only" | "admin";

export interface RoleFlags {
  is_admin: boolean;
  is_read_only: boolean;
}

export interface RoleChange {
  flag: "admin" | "readOnly";
  state: boolean;
}

export const USER_ROLES: ReadonlyArray<{ value: UserRole; label: string; description: string }> = [
  { value: "user", label: "User", description: "Can view and publish crates" },
  { value: "read-only", label: "Read-only", description: "Can only view crates" },
  { value: "admin", label: "Admin", description: "Full access to all features" },
];

export function roleLabel(role: UserRole): string {
  return USER_ROLES.find((r) => r.value === role)?.label ?? role;
}

export function roleFlags(role: UserRole): RoleFlags {
  return { is_admin: role === "admin", is_read_only: role === "read-only" };
}

/** Admin rights override the read-only flag in the backend, so admins count as admins. */
export function roleOf(user: RoleFlags): UserRole {
  if (user.is_admin) return "admin";
  return user.is_read_only ? "read-only" : "user";
}

/** API flag changes that give the user exactly the target role. */
export function roleChanges(user: RoleFlags, target: UserRole): RoleChange[] {
  const wanted = roleFlags(target);
  const changes: RoleChange[] = [];
  if (user.is_admin !== wanted.is_admin) {
    changes.push({ flag: "admin", state: wanted.is_admin });
  }
  if (user.is_read_only !== wanted.is_read_only) {
    changes.push({ flag: "readOnly", state: wanted.is_read_only });
  }
  return changes;
}

/**
 * Whether the user may change a crate (owners, crate users and groups, yank, doc builds).
 * Admins are exempt from the read-only restriction, as in the backend.
 */
export function canModifyCrate(user: RoleFlags, isOwner: boolean): boolean {
  if (user.is_admin) return true;
  return isOwner && !user.is_read_only;
}
