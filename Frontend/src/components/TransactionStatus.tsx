'use client'

import React from 'react'
import { TransactionStatusType } from '@/types'

interface TransactionStatusProps {
  status: TransactionStatusType
  message: string
  reference?: string
  onDismiss?: () => void
}

export const TransactionStatus: React.FC<TransactionStatusProps> = ({
  status,
  message,
  reference,
  onDismiss,
}) => {
  const statusConfig = {
    pending: {
      icon: (
        <svg className="animate-spin h-5 w-5 text-blue-500" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
        </svg>
      ),
      bgColor: 'bg-blue-50',
      borderColor: 'border-blue-200',
      textColor: 'text-blue-800',
      label: 'Processing',
    },
    success: {
      icon: (
        <svg className="h-5 w-5 text-green-500" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
          <path stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M5 13l4 4L19 7"></path>
        </svg>
      ),
      bgColor: 'bg-green-50',
      borderColor: 'border-green-200',
      textColor: 'text-green-800',
      label: 'Success',
    },
    error: {
      icon: (
        <svg className="h-5 w-5 text-red-500" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
          <path stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12"></path>
        </svg>
      ),
      bgColor: 'bg-red-50',
      borderColor: 'border-red-200',
      textColor: 'text-red-800',
      label: 'Error',
    },
    dismissed: {
      icon: null,
      bgColor: 'bg-gray-50',
      borderColor: 'border-gray-200',
      textColor: 'text-gray-500',
      label: 'Dismissed',
    },
  }

  const config = statusConfig[status]

  if (status === 'dismissed') return null

  return (
    <div
      role="alert"
      aria-live={status === 'error' ? 'assertive' : 'polite'}
      className={`${config.bgColor} ${config.borderColor} border rounded-lg p-4 shadow-lg mb-2 flex items-start gap-3 min-w-[320px] max-w-md`}
    >
      <div className="flex-shrink-0 mt-0.5">{config.icon}</div>
      <div className="flex-1">
        <p className={`font-medium ${config.textColor}`}>{config.label}</p>
        <p className={`text-sm ${config.textColor} opacity-90 mt-1`}>{message}</p>
        {reference && (
          <p className="text-xs text-gray-500 mt-2">
            Reference: <span className="font-mono">{reference}</span>
          </p>
        )}
      </div>
      {onDismiss && (
        <button
          onClick={onDismiss}
          className="flex-shrink-0 p-1 rounded-md hover:bg-black/5 transition-colors focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-gray-500"
          aria-label="Dismiss notification"
        >
          <svg className="h-4 w-4 text-gray-500" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
            <path stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12"></path>
          </svg>
        </button>
      )}
    </div>
  )
}

export default TransactionStatus