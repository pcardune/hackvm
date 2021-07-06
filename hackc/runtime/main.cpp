#include <stdio.h>
#include <stdlib.h>
#include <thread>
#include <SDL2/SDL.h>

extern "C" long int hack_sys_init(long int**);
void show_window(long int*, long int*);

const int SCREEN_WIDTH = 512;
const int SCREEN_HEIGHT = 256;
const int SCALE = 4;

int main() {
  long int* ram = NULL;

  std::thread runner(hack_sys_init, &ram);
  // hack_sys_init(&ram);
  while (ram == NULL) {}
  show_window(ram + 16384, ram + 24576);

  printf("output is (at %p):\n", ram);
  for (int i = 0; i < 16; i++) {
    printf("  [%2i] = %ld\n", i, ram[i]);
  }

  // runner.join();
  exit(EXIT_SUCCESS);
}


// ------------------------ SDL STUFF --------------------

//The window we'll be rendering to
SDL_Window* gWindow = NULL;

SDL_Renderer* gRenderer = NULL;

//The surface contained by the window
SDL_Surface* gScreenSurface = NULL;
SDL_Surface* gDrawingSurface = NULL;

bool init()
{
  //Initialization flag
  bool success = true;

  //Initialize SDL
  if (SDL_Init(SDL_INIT_VIDEO) < 0)
  {
    printf("SDL could not initialize! SDL_Error: %s\n", SDL_GetError());
    success = false;
  }
  else
  {
    //Create window
    if (SDL_CreateWindowAndRenderer(
      SCREEN_WIDTH * SCALE,
      SCREEN_HEIGHT * SCALE,
      SDL_WINDOW_SHOWN,
      &gWindow,
      &gRenderer
    ) != 0)
    {
      printf("Window could not be created! SDL_Error: %s\n", SDL_GetError());
      success = false;
    }
    else
    {
      //Get window surface
      gScreenSurface = SDL_GetWindowSurface(gWindow);
      SDL_RenderSetLogicalSize(gRenderer, SCREEN_WIDTH, SCREEN_HEIGHT);

      gDrawingSurface = SDL_CreateRGBSurface(
        0,
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        32,
        0, 0, 0, 0
      );
      if (gDrawingSurface == NULL) {
        printf("Failed creating drawing surface! SDL_Error: %s\n", SDL_GetError());
        success = false;
      }
    }
  }

  return success;
}

void closeWindow()
{
  SDL_FreeSurface(gDrawingSurface);
  gDrawingSurface = NULL;

  SDL_DestroyRenderer(gRenderer);
  gRenderer = NULL;

  //Destroy window
  SDL_DestroyWindow(gWindow);
  gWindow = NULL;

  //Quit SDL subsystems
  SDL_Quit();
}

void show_window(long int* screenStart, long int* keyboard) {
  //Start up SDL and create window
  if (!init())
  {
    printf("Failed to initialize!\n");
  }
  else
  {

    //Main loop flag
    bool quit = false;

    //Event handler
    SDL_Event e;

    //While application is running
    while (!quit)
    {
      //Handle events on queue
      while (SDL_PollEvent(&e) != 0)
      {
        //User requests quit
        if (e.type == SDL_QUIT)
        {
          quit = true;
        }
        //User presses a key
        else if (e.type == SDL_KEYDOWN)
        {
          printf("Pressed key %s\n", SDL_GetKeyName(e.key.keysym.sym));
          if (e.key.keysym.sym == SDLK_DOWN) {
            //Fill the surface white
            SDL_FillRect(gScreenSurface, NULL, SDL_MapRGB(gScreenSurface->format, 0xAA, 0xFF, 0x33));
          }
          long int key = 0;
          switch (e.key.keysym.sym)
          {
          case SDLK_SPACE:
            key = 32;
            break;
          case SDLK_LEFT:
            key = 130;
            break;
          case SDLK_UP:
            key = 131;
            break;
          case SDLK_RIGHT:
            key = 132;
            break;
          case SDLK_DOWN:
            key = 133;
            break;
          }
          if (key != 0) {
            printf("Setting key to %li\n", key);
            *keyboard = key;
          }
        }
        else if (e.type == SDL_KEYUP)
        {
          *keyboard = 0;
        }
      }

      SDL_LockSurface(gDrawingSurface);
      Uint32* pixels = (Uint32*)gDrawingSurface->pixels;
      Uint32 black = SDL_MapRGB(gDrawingSurface->format, 0x00, 0x00, 0x00);
      Uint32 white = SDL_MapRGB(gDrawingSurface->format, 0xFF, 0xFF, 0xFF);

      int p = 0;
      for (int i = 0; i < SCREEN_HEIGHT * SCREEN_WIDTH / 16; i++) {
        Uint16 block = screenStart[i];
        for (int j = 0; j < 16; j++) {
          if ((block & 1 << j) > 0) {
            pixels[p++] = black;
          }
          else {
            pixels[p++] = white;
          }
        }
      }
      SDL_UnlockSurface(gDrawingSurface);
      // printf("pixel is: %xd. bits per pixel: %d\n", index, gScreenSurface->format->BitsPerPixel);

      SDL_Rect srcRect;
      srcRect.x = 0;
      srcRect.y = 0;
      srcRect.w = SCREEN_WIDTH;
      srcRect.h = SCREEN_HEIGHT;
      SDL_Rect dstRect = srcRect;
      dstRect.w *= SCALE;
      dstRect.h *= SCALE;

      if (SDL_BlitScaled(gDrawingSurface, &srcRect, gScreenSurface, &dstRect) != 0) {
        printf("Failed blitting surface! SDL_Error: %s\n", SDL_GetError());
      };

      //Update the surface
      SDL_UpdateWindowSurface(gWindow);

    }

  }

  closeWindow();
}
