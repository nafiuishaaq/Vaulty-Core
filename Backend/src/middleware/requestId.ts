import { Request, Response, NextFunction } from 'express';
import { randomUUID } from 'crypto';

declare global {
  namespace Express {
    interface Request {
      requestId: string;
    }
  }
}

export const requestId = (req: Request, res: Response, next: NextFunction): void => {
  req.requestId = randomUUID();
  res.setHeader('X-Request-Id', req.requestId);
  next();
};