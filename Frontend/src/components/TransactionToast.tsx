'use client'

import React, { useEffect, useCallback } from 'react'
import { useAppStore } from '@/stores'
import TransactionStatus from './TransactionStatus'
import { TransactionNotification } from '@/types'

export const TransactionToast: React.FC = () => {
  const { transactionNotifications, dismissTransactionNotification, removeTransactionNotification } = useAppStore()

  // Filter out dismissed notifications that are older than 5 seconds to clean up
  useEffect(() => {
    const cleanupInterval = setInterval(() => {
      const now = new Date()
      transactionNotifications.forEach((notification) => {
        if (notification.status === 'dismissed') {
          const updatedAt = new Date(notification.updatedAt)
          const diff = now.getTime() - updatedAt.getTime()
          if (diff > 5000) {
            removeTransactionNotification(notification.id)
          }
        }
      })
    }, 1000)

    return () => clearInterval(cleanupInterval)
  }, [transactionNotifications, removeTransactionNotification])

  // Auto-dismiss success and error notifications after 5 seconds
  useEffect(() => {
    const dismissTimeouts: NodeJS.Timeout[] = []
    
    transactionNotifications.forEach((notification) => {
      if (notification.status === 'success' || notification.status === 'error') {
        const updatedAt = new Date(notification.updatedAt)
        const now = new Date()
        const diff = now.getTime() - updatedAt.getTime()
        
        if (diff < 100) { // Only set timeout if just updated
          const timeout = setTimeout(() => {
            dismissTransactionNotification(notification.id)
          }, 5000)
          dismissTimeouts.push(timeout)
        }
      }
    })

    return () => dismissTimeouts.forEach(clearTimeout)
  }, [transactionNotifications, dismissTransactionNotification])

  // Keyboard dismissal (Escape key)
  const handleKeyDown = useCallback((event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      const activeNotifications = transactionNotifications.filter(
        n => n.status !== 'dismissed'
      )
      if (activeNotifications.length > 0) {
        // Dismiss the most recent notification first
        const mostRecent = activeNotifications[activeNotifications.length - 1]
        dismissTransactionNotification(mostRecent.id)
      }
    }
  }, [transactionNotifications, dismissTransactionNotification])

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [handleKeyDown])

  // Screen reader announcements
  useEffect(() => {
    const announcements: string[] = []
    transactionNotifications.forEach((notification) => {
      if (notification.status === 'pending') {
        announcements.push(`${notification.action}: ${notification.message}`)
      } else if (notification.status === 'success') {
        announcements.push(`${notification.action} successful: ${notification.message}`)
      } else if (notification.status === 'error') {
        announcements.push(`${notification.action} failed: ${notification.message}`)
      }
    })

    if (announcements.length > 0) {
      const announcer = document.getElementById('sr-announcer')
      if (announcer) {
        announcer.textContent = announcements.join('. ')
        // Clear after announcement
        setTimeout(() => {
          if (announcer) announcer.textContent = ''
        }, 1000)
      }
    }
  }, [transactionNotifications])

  const activeNotifications = transactionNotifications.filter(
    (notification): notification is TransactionNotification => 
      notification.status !== 'dismissed'
  )

  if (activeNotifications.length === 0) return null

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col-reverse">
      {activeNotifications.map((notification) => (
        <TransactionStatus
          key={notification.id}
          status={notification.status}
          message={notification.message}
          reference={notification.reference}
          onDismiss={() => dismissTransactionNotification(notification.id)}
        />
      ))}
    </div>
  )
}

export default TransactionToast