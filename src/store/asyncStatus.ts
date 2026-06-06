/** Status of an async data domain. Drives skeleton / error+retry / content UI. */
export type AsyncStatus = 'idle' | 'loading' | 'error' | 'success'
