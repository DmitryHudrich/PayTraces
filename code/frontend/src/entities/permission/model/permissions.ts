/** Permission names as returned by the C# API's authorization stack. */
export const Permission = {
  CaseCreate: 'CaseCreate',
  CaseRead: 'CaseRead',
  CaseUpdate: 'CaseUpdate',
  CaseClose: 'CaseClose',
  CaseAssign: 'CaseAssign',
  CaseAddressAdd: 'CaseAddressAdd',
  CaseAddressRemove: 'CaseAddressRemove',
  CaseNoteAdd: 'CaseNoteAdd',
  ViewCreate: 'ViewCreate',
  ViewRead: 'ViewRead',
  ViewUpdate: 'ViewUpdate',
  ViewDelete: 'ViewDelete',
  ViewManageSharing: 'ViewManageSharing',
  LabelCreate: 'LabelCreate',
  LabelApply: 'LabelApply',
  GroupCreate: 'GroupCreate',
  GroupUpdate: 'GroupUpdate',
  GroupDelete: 'GroupDelete',
} as const

export type PermissionName = (typeof Permission)[keyof typeof Permission]

export class PermissionSet {
  private readonly names: ReadonlySet<string>

  constructor(names: Iterable<string>) {
    this.names = new Set(names)
  }

  can(permission: PermissionName): boolean {
    return this.names.has(permission)
  }

  canAny(...permissions: PermissionName[]): boolean {
    return permissions.some((permission) => this.names.has(permission))
  }
}
