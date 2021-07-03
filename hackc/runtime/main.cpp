#include <stdio.h>
#include <stdlib.h>
#include <thread>
#include <SDL2/SDL.h>

extern "C" long int hack_sys_init(long int**);
void show_window(long int*);

const int SCREEN_WIDTH = 512;
const int SCREEN_HEIGHT = 256;

int main() {
  long int* ram = NULL;

  std::thread runner(hack_sys_init, &ram);
  // hack_sys_init(&ram);
  while (ram == NULL) {}
  show_window(ram + 16384);

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

//The surface contained by the window
SDL_Surface* gScreenSurface = NULL;

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
    gWindow = SDL_CreateWindow("SDL Tutorial", SDL_WINDOWPOS_UNDEFINED, SDL_WINDOWPOS_UNDEFINED, SCREEN_WIDTH, SCREEN_HEIGHT, SDL_WINDOW_SHOWN);
    if (gWindow == NULL)
    {
      printf("Window could not be created! SDL_Error: %s\n", SDL_GetError());
      success = false;
    }
    else
    {
      //Get window surface
      gScreenSurface = SDL_GetWindowSurface(gWindow);
    }
  }

  return success;
}

void closeWindow()
{
  //Destroy window
  SDL_DestroyWindow(gWindow);
  gWindow = NULL;

  //Quit SDL subsystems
  SDL_Quit();
}

void show_window(long int* ram) {
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
        }
      }

      SDL_LockSurface(gScreenSurface);
      Uint32* pixels = (Uint32*)gScreenSurface->pixels;
      Uint32 black = SDL_MapRGB(gScreenSurface->format, 0xFF, 0x11, 0x22);
      Uint32 white = SDL_MapRGB(gScreenSurface->format, 0x55, 0xFF, 0xFF);

      int p = 0;
      for (int i = 0; i < SCREEN_HEIGHT * SCREEN_WIDTH / 16; i++) {
        Uint16 block = ram[i];
        for (int j = 0; j < 16; j++) {
          if ((block & 1 << j) > 0) {
            pixels[p++] = black;
          }
          else {
            pixels[p++] = white;
          }
        }
      }
      SDL_UnlockSurface(gScreenSurface);
      // printf("pixel is: %xd. bits per pixel: %d\n", index, gScreenSurface->format->BitsPerPixel);

      //Update the surface
      SDL_UpdateWindowSurface(gWindow);

    }

  }

  closeWindow();
}
