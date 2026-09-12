import type { ChannelKind, RoleId } from './types.ts'
import { makeId } from './ids.ts'

export type RoleDef = {
  id: RoleId
  label: string
  hint: string
  contract: string
  defaultChannel: ChannelKind
  builtin: boolean
}

export const ROLE_STORAGE_KEY = 'ad-bridge-roles'

export const BUILTIN_ROLES: RoleDef[] = [
  {
    id: 'developer',
    label: '开发者',
    hint: '只做本片，做完即停',
    contract: '必须只改当前这一片需要的文件，做完后停下来。不得开新需求，不得自己给自己过闸。',
    defaultChannel: 'cursorSdk',
    builtin: true
  },
  {
    id: 'reviewer',
    label: '审查者',
    hint: '出书面结论和通过票',
    contract: '必须针对本轮改动给出书面意见，并以 VERDICT: PASS 或 VERDICT: FAIL 结尾。不得直接改业务代码。',
    defaultChannel: 'codexApp',
    builtin: true
  },
  {
    id: 'pm',
    label: '项目经理',
    hint: '写任务书，不改代码',
    contract: '必须写清本片任务、范围和完成标准。不得改代码，不得下审查结论。',
    defaultChannel: 'human',
    builtin: true
  }
]

export const ROLE_CONTRACTS: Record<string, string> = Object.fromEntries(
  BUILTIN_ROLES.map((role) => [role.id, role.contract])
)

type StorageLike = {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
}

function storage(): StorageLike | null {
  try {
    if (typeof localStorage !== 'undefined') return localStorage
  } catch {
    /* ignore */
  }
  return null
}

export function loadCustomRoles(store: StorageLike | null = storage()): RoleDef[] {
  if (!store) return []
  try {
    const raw = store.getItem(ROLE_STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as RoleDef[]
    if (!Array.isArray(parsed)) return []
    return parsed
      .filter((item) => item && item.id && item.label && !BUILTIN_ROLES.some((role) => role.id === item.id))
      .map((item) => ({
        id: item.id,
        label: item.label,
        hint: item.hint || '自定义角色',
        contract: item.contract || '',
        defaultChannel: item.defaultChannel || 'pty',
        builtin: false
      }))
  } catch {
    return []
  }
}

export function persistCustomRoles(roles: RoleDef[], store: StorageLike | null = storage()) {
  if (!store) return
  try {
    store.setItem(ROLE_STORAGE_KEY, JSON.stringify(roles.filter((item) => !item.builtin)))
  } catch {
    /* ignore quota */
  }
}

export function allRoles(custom: RoleDef[] = loadCustomRoles()): RoleDef[] {
  return [...BUILTIN_ROLES, ...custom]
}

export function findRole(id: RoleId, custom: RoleDef[] = loadCustomRoles()): RoleDef | undefined {
  return allRoles(custom).find((item) => item.id === id)
}

export function roleLabel(role: RoleId, custom: RoleDef[] = loadCustomRoles()) {
  return findRole(role, custom)?.label || role
}

export function roleHint(role: RoleId, custom: RoleDef[] = loadCustomRoles()) {
  return findRole(role, custom)?.hint || ''
}

export function roleContract(role: RoleId, custom: RoleDef[] = loadCustomRoles()) {
  return findRole(role, custom)?.contract || ''
}

export function addCustomRole(label: string, custom: RoleDef[] = loadCustomRoles(), store: StorageLike | null = storage()) {
  const name = label.trim()
  if (!name) return { roles: custom, role: null as RoleDef | null, error: '给角色起个名字' }
  if (allRoles(custom).some((item) => item.label === name)) {
    return { roles: custom, role: null as RoleDef | null, error: '已有同名角色' }
  }
  const role: RoleDef = {
    id: makeId('role'),
    label: name,
    hint: '自定义角色',
    contract: `你是${name}。按流程完成自己的步骤后停下，不要越权。`,
    defaultChannel: 'pty',
    builtin: false
  }
  const roles = [...custom, role]
  persistCustomRoles(roles, store)
  return { roles, role, error: '' }
}
